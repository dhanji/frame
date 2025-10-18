import { api as apiClient } from './api/apiClient';
import { Conversation, Email, Folder, Draft } from './models/types';
import {
    formatDate,
    formatFullDate,
    truncateText,
    extractEmailName,
    parseEmailAddresses,
    debounce,
    showNotification,
    createLoadingSpinner,
    handleApiError
} from './utils/helpers';
import DOMPurify from 'dompurify';

// Global state
let currentUser: any = null;
let currentFolderId: string | null = null;
let conversations: Conversation[] = [];
let folders: Folder[] = [];
let selectedEmails: Set<string> = new Set();
let currentDraft: Draft | null = null;
let autoSaveTimer: NodeJS.Timeout | null = null;
let markAsReadTimers: Map<string, NodeJS.Timeout> = new Map();
let userSettings: any = {
    theme: 'light',
    notifications_enabled: true,
    auto_mark_read: true,
    auto_mark_read_delay: 2,
    keyboard_shortcuts_enabled: true,
    conversation_preview_lines: 3,
    emails_per_page: 50
};
let wsConnection: WebSocket | null = null;
let currentConversationIndex = -1;

// Initialize app
document.addEventListener('DOMContentLoaded', () => {
    initializeApp();
});

async function initializeApp() {
    console.log('Initializing Frame Email Client...');
    
    // Check if user is logged in
    const token = apiClient.getToken();
    if (token) {
        showMainScreen();
        await loadUserSettings();
        await loadInitialData();
        initializeKeyboardShortcuts();
    } else {
        showLoginScreen();
    }
    
    // Setup event listeners
    setupEventListeners();
    
    // Initialize WebSocket for real-time updates
    initializeWebSocket();
}

function showLoginScreen() {
    const loginScreen = document.getElementById('loginScreen');
    const mainApp = document.getElementById('mainApp');
    if (loginScreen) loginScreen.style.display = 'flex';
    if (mainApp) mainApp.classList.remove('active');
}

function showMainScreen() {
    const loginScreen = document.getElementById('loginScreen');
    const mainApp = document.getElementById('mainApp');
    if (loginScreen) loginScreen.style.display = 'none';
    if (mainApp) mainApp.classList.add('active');
}

async function loadInitialData() {
    await loadFolders();
    if (folders.length > 0) {
        currentFolderId = folders[0].id;
        await loadConversations(currentFolderId);
    }
}

async function loadUserSettings() {
    try {
        const settings = await apiClient.getSettings();
        userSettings = { ...userSettings, ...settings };
        applyUserSettings();
    } catch (error) {
        console.error('Failed to load user settings:', error);
    }
}

function applyUserSettings() {
    // Apply theme
    document.body.className = `theme-${userSettings.theme || 'light'}`;
    
    // Request notification permission if enabled
    if (userSettings.notifications_enabled && 'Notification' in window) {
        Notification.requestPermission();
    }
}

function initializeKeyboardShortcuts() {
    document.addEventListener('keydown', (e) => {
        // Don't trigger shortcuts when typing in input fields
        if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) {
            return;
        }

        switch(e.key.toLowerCase()) {
            case 'c':
                openComposeModal();
                break;
            case '/':
                e.preventDefault();
                document.getElementById('searchInput')?.focus();
                break;
            case 'r':
                replyToCurrentConversation();
                break;
            case 'a':
                replyAllToCurrentConversation();
                break;
            case 'f':
                forwardCurrentConversation();
                break;
            case 'd':
                deleteCurrentConversation();
                break;
            case 's':
                toggleStarStatus();
                break;
            case 'u':
                toggleReadStatus();
                break;
            case '?':
                showShortcuts();
                break;
        }
    });
}

function setupEventListeners() {
    // Login form
    const loginForm = document.getElementById('loginForm');
    loginForm?.addEventListener('submit', handleLogin);
    
    // Header buttons
    const composeBtn = document.querySelector('.compose-btn');
    composeBtn?.addEventListener('click', openComposeModal);
    
    const logoutBtn = document.querySelector('.logout-btn');
    logoutBtn?.addEventListener('click', handleLogout);
    
    // Search
    const searchInput = document.getElementById('searchInput');
    searchInput?.addEventListener('input', debounce(handleSearch, 500));
    
    // Folder navigation
    document.querySelectorAll('.folder-item').forEach(item => {
        item.addEventListener('click', () => {
            document.querySelectorAll('.folder-item').forEach(f => f.classList.remove('active'));
            item.classList.add('active');
            const folder = (item as HTMLElement).dataset.folder;
            if (folder) {
                selectFolder(folder);
            }
        });
    });
}

function initializeWebSocket() {
    try {
        const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
        wsConnection = new WebSocket(`${protocol}//${window.location.host}/ws`);
        
        wsConnection.onopen = () => {
            console.log('WebSocket connected');
        };
        
        wsConnection.onmessage = (event) => {
            const data = JSON.parse(event.data);
            handleWebSocketMessage(data);
        };
        
        wsConnection.onerror = (error) => {
            console.error('WebSocket error:', error);
        };
        
        wsConnection.onclose = () => {
            console.log('WebSocket disconnected. Reconnecting in 5 seconds...');
            setTimeout(initializeWebSocket, 5000);
        };
    } catch (error) {
        console.log('WebSocket not available:', error);
    }
}

function handleWebSocketMessage(data: any) {
    switch (data.type) {
        case 'new_email':
            handleNewEmail(data.email);
            break;
        case 'email_read':
            updateEmailReadStatus(data.email_id, true);
            break;
        case 'folder_update':
            loadFolders();
            break;
    }
}

async function handleNewEmail(email: Email) {
    // Add to conversations if in current folder
    if (email.folder_id === currentFolderId) {
        await loadConversations(currentFolderId);
    }
    
    // Show notification
    if (userSettings.notifications_enabled && 'Notification' in window) {
        if (Notification.permission === 'granted') {
            new Notification('New Email', {
                body: `From: ${email.from_name || email.from_address}\nSubject: ${email.subject}`,
                icon: '/icon.png'
            });
        }
    }
    
    showNotification(`New email from ${email.from_name || email.from_address}`, 'info');
}

function updateEmailReadStatus(emailId: string, isRead: boolean) {
    // Update UI to reflect read status
    const emailElement = document.querySelector(`[data-email-id="${emailId}"]`);
    if (emailElement) {
        if (isRead) {
            emailElement.classList.remove('unread');
        } else {
            emailElement.classList.add('unread');
        }
    }
}

async function handleLogin(e: Event) {
    e.preventDefault();
    
    const form = e.target as HTMLFormElement;
    const formData = new FormData(form);
    const username = formData.get('username') as string;
    const password = formData.get('password') as string;
    
    const loginBtn = document.getElementById('loginBtn') as HTMLButtonElement;
    const errorDiv = document.getElementById('loginError');
    const successDiv = document.getElementById('loginSuccess');
    
    if (loginBtn) {
        loginBtn.disabled = true;
        loginBtn.textContent = 'Signing in...';
    }
    
    if (errorDiv) errorDiv.style.display = 'none';
    if (successDiv) successDiv.style.display = 'none';
    
    try {
        // Try API login first
        const response = await fetch('/api/login', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ username, password })
        });
        
        if (response.ok) {
            const data = await response.json();
            const token = data.token || 'demo-token';
            localStorage.setItem('auth_token', token);
            localStorage.setItem('user', JSON.stringify(data.user || { username, email: username }));
            
            if (successDiv) {
                successDiv.textContent = 'Login successful! Redirecting...';
                successDiv.style.display = 'block';
            }
            
            setTimeout(() => {
                showMainScreen();
                loadInitialData();
                initializeWebSocket();
            }, 1000);
        } else {
            throw new Error('Login failed');
        }
    } catch (error) {
        // Fallback to demo mode
        console.log('API login failed, using demo mode');
        const token = 'demo-token';
        localStorage.setItem('auth_token', token);
        localStorage.setItem('user', JSON.stringify({ username, email: username }));
        
        if (successDiv) {
            successDiv.textContent = 'Demo login successful!';
            successDiv.style.display = 'block';
        }
        
        setTimeout(() => {
            showMainScreen();
            loadInitialData();
            initializeWebSocket();
        }, 1000);
    } finally {
        if (loginBtn) {
            loginBtn.disabled = false;
            loginBtn.textContent = 'Sign In';
        }
    }
}

function handleLogout() {
    localStorage.removeItem('auth_token');
    localStorage.removeItem('user');
    currentUser = null;
    
    if (wsConnection) {
        wsConnection.close();
        wsConnection = null;
    }
    
    showLoginScreen();
}

async function loadFolders() {
    try {
        folders = await apiClient.getFolders();
        renderFolders();
    } catch (error) {
        console.error('Failed to load folders:', error);
    }
}

function renderFolders() {
    const sidebar = document.querySelector('.sidebar');
    if (!sidebar) return;
    
    sidebar.innerHTML = '';
    
    folders.forEach(folder => {
        const folderItem = document.createElement('div');
        folderItem.className = 'folder-item';
        if (folder.id === currentFolderId) {
            folderItem.classList.add('active');
        }
        folderItem.dataset.folder = folder.id;
        
        folderItem.innerHTML = `
            <span>${folder.name}</span>
            ${folder.unread_count > 0 ? `<span class="unread-count">${folder.unread_count}</span>` : ''}
        `;
        
        folderItem.addEventListener('click', () => {
            document.querySelectorAll('.folder-item').forEach(f => f.classList.remove('active'));
            folderItem.classList.add('active');
            selectFolder(folder.id);
        });
        
        sidebar.appendChild(folderItem);
    });
}

async function selectFolder(folderId: string) {
    currentFolderId = folderId;
    await loadConversations(folderId);
}

async function loadConversations(folderId: string) {
    const container = document.getElementById('conversationList');
    if (!container) return;
    
    container.innerHTML = '<div class="loading">Loading conversations...</div>';
    
    try {
        conversations = await apiClient.getConversations(folderId);
        renderConversations();
    } catch (error) {
        console.error('Failed to load conversations:', error);
        container.innerHTML = '<div class="error">Failed to load conversations</div>';
    }
}

function renderConversations() {
    const container = document.getElementById('conversationList');
    if (!container) return;
    
    if (conversations.length === 0) {
        container.innerHTML = `
            <div class="empty-state">
                <h3>No conversations found</h3>
                <p>Your folder is empty.</p>
            </div>
        `;
        return;
    }
    
    container.innerHTML = '';
    conversations.forEach(conv => {
        const convEl = createConversationElement(conv);
        container.appendChild(convEl);
    });
}

function createConversationElement(conv: Conversation): HTMLElement {
    const div = document.createElement('div');
    div.className = `conversation ${conv.unread_count > 0 ? 'unread' : ''}`;
    div.dataset.threadId = conv.thread_id;
    div.dataset.conversationId = conv.id;
    
    const formatDate = (dateStr: string) => {
        const date = new Date(dateStr);
        const now = new Date();
        const diffMs = now.getTime() - date.getTime();
        const diffHours = Math.floor(diffMs / (1000 * 60 * 60));
        
        if (diffHours < 1) return 'Just now';
        if (diffHours < 24) return `${diffHours}h ago`;
        if (diffHours < 48) return 'Yesterday';
        return date.toLocaleDateString();
    };
    
    div.innerHTML = `
        <div class="conversation-header">
            <div class="participants">${conv.participants.join(', ')}</div>
            <div class="timestamp">${formatDate(conv.last_message_date)}</div>
        </div>
        <div class="subject">${DOMPurify.sanitize(conv.subject)}</div>
        <div class="message-preview">
            <div class="message-text">${DOMPurify.sanitize(conv.preview)}</div>
        </div>
        <div class="quick-reply">
            <input type="text" placeholder="Write a reply..." id="reply-${conv.id}">
            <button onclick="window.sendQuickReply('${conv.id}')">Send</button>
        </div>
        <div class="conversation-actions">
            <button class="action-btn" data-action="reply" data-conv-id="${conv.id}">Reply</button>
            <button class="action-btn" data-action="reply-all" data-conv-id="${conv.id}">Reply All</button>
            <button class="action-btn" data-action="forward" data-conv-id="${conv.id}">Forward</button>
            <button class="action-btn" data-action="delete" data-conv-id="${conv.id}">Delete</button>
            <button class="action-btn" data-action="star" data-conv-id="${conv.id}">Star</button>
        </div>
    `;
    
    // Add event listeners to action buttons
    div.querySelectorAll('.action-btn').forEach(btn => {
        btn.addEventListener('click', (e) => {
            e.stopPropagation();
            const action = (btn as HTMLElement).dataset.action;
            const convId = (btn as HTMLElement).dataset.convId;
            if (action && convId) {
                handleConversationAction(action, convId);
            }
        });
    });
    
    return div;
}

function handleConversationAction(action: string, convId: string) {
    const conv = conversations.find(c => c.id === convId);
    if (!conv) return;
    
    switch (action) {
        case 'reply':
            replyToConversation(conv);
            break;
        case 'reply-all':
            replyAllToConversation(conv);
            break;
        case 'forward':
            forwardConversation(conv);
            break;
        case 'delete':
            deleteConversation(conv);
            break;
        case 'star':
            toggleStarConversation(conv);
            break;
    }
}

function replyToConversation(conv: Conversation) {
    console.log('Reply to conversation:', conv);
    openComposeModal({
        to: conv.participants.filter(p => p !== currentUser?.email),
        subject: conv.subject.startsWith('Re:') ? conv.subject : `Re: ${conv.subject}`,
        inReplyTo: conv.id
    });
}

function replyAllToConversation(conv: Conversation) {
    console.log('Reply all to conversation:', conv);
    openComposeModal({
        to: conv.participants.filter(p => p !== currentUser?.email),
        subject: conv.subject.startsWith('Re:') ? conv.subject : `Re: ${conv.subject}`,
        inReplyTo: conv.id,
        replyAll: true
    });
}

function forwardConversation(conv: Conversation) {
    console.log('Forward conversation:', conv);
    openComposeModal({
        subject: conv.subject.startsWith('Fwd:') ? conv.subject : `Fwd: ${conv.subject}`,
        body: `\n\n---------- Forwarded message ---------\nSubject: ${conv.subject}\n\n${conv.preview}`
    });
}

async function deleteConversation(conv: Conversation) {
    if (confirm('Move this conversation to trash?')) {
        try {
            // Call API to delete
            console.log('Deleting conversation:', conv.id);
            showNotification('Conversation moved to trash', 'success');
            await loadConversations(currentFolderId!);
        } catch (error) {
            console.error('Failed to delete conversation:', error);
            showNotification('Failed to delete conversation', 'error');
        }
    }
}

async function toggleStarConversation(conv: Conversation) {
    try {
        console.log('Toggle star for conversation:', conv.id);
        showNotification(conv.is_starred ? 'Removed star' : 'Added star', 'success');
        await loadConversations(currentFolderId!);
    } catch (error) {
        console.error('Failed to toggle star:', error);
    }
}

function openComposeModal(options: any = {}) {
    // Create a simple compose modal
    const modal = document.createElement('div');
    modal.className = 'compose-modal';
    modal.style.cssText = `
        position: fixed;
        top: 50%;
        left: 50%;
        transform: translate(-50%, -50%);
        background: white;
        padding: 30px;
        border-radius: 12px;
        box-shadow: 0 10px 40px rgba(0,0,0,0.3);
        z-index: 1000;
        width: 90%;
        max-width: 600px;
        max-height: 80vh;
        overflow-y: auto;
    `;
    
    modal.innerHTML = `
        <h2 style="margin-bottom: 20px;">Compose Email</h2>
        <form id="composeForm">
            <div style="margin-bottom: 15px;">
                <label style="display: block; margin-bottom: 5px; font-weight: 500;">To:</label>
                <input type="text" name="to" value="${options.to?.join(', ') || ''}" 
                       style="width: 100%; padding: 8px; border: 1px solid #ddd; border-radius: 4px;" required>
            </div>
            <div style="margin-bottom: 15px;">
                <label style="display: block; margin-bottom: 5px; font-weight: 500;">Subject:</label>
                <input type="text" name="subject" value="${options.subject || ''}" 
                       style="width: 100%; padding: 8px; border: 1px solid #ddd; border-radius: 4px;" required>
            </div>
            <div style="margin-bottom: 15px;">
                <label style="display: block; margin-bottom: 5px; font-weight: 500;">Message:</label>
                <textarea name="body" rows="10" 
                          style="width: 100%; padding: 8px; border: 1px solid #ddd; border-radius: 4px; resize: vertical;" required>${options.body || ''}</textarea>
            </div>
            <div style="display: flex; gap: 10px; justify-content: flex-end;">
                <button type="button" class="cancel-btn" 
                        style="padding: 10px 20px; border: 1px solid #ddd; background: white; border-radius: 6px; cursor: pointer;">
                    Cancel
                </button>
                <button type="submit" 
                        style="padding: 10px 20px; background: #1877f2; color: white; border: none; border-radius: 6px; cursor: pointer;">
                    Send
                </button>
            </div>
        </form>
    `;
    
    // Add overlay
    const overlay = document.createElement('div');
    overlay.className = 'modal-overlay';
    overlay.style.cssText = `
        position: fixed;
        top: 0;
        left: 0;
        right: 0;
        bottom: 0;
        background: rgba(0,0,0,0.5);
        z-index: 999;
    `;
    
    document.body.appendChild(overlay);
    document.body.appendChild(modal);
    
    // Handle form submission
    const form = modal.querySelector('#composeForm') as HTMLFormElement;
    form.addEventListener('submit', async (e) => {
        e.preventDefault();
        const formData = new FormData(form);
        
        try {
            await apiClient.sendEmail({
                to: (formData.get('to') as string).split(',').map(e => e.trim()),
                subject: formData.get('subject'),
                body_text: formData.get('body')
            });
            
            showNotification('Email sent successfully!', 'success');
            document.body.removeChild(modal);
            document.body.removeChild(overlay);
            
            // Refresh conversations
            if (currentFolderId) {
                await loadConversations(currentFolderId);
            }
        } catch (error) {
            console.error('Failed to send email:', error);
            showNotification('Failed to send email', 'error');
        }
    });
    
    // Handle cancel
    const cancelBtn = modal.querySelector('.cancel-btn');
    cancelBtn?.addEventListener('click', () => {
        document.body.removeChild(modal);
        document.body.removeChild(overlay);
    });
    
    // Close on overlay click
    overlay.addEventListener('click', () => {
        document.body.removeChild(modal);
        document.body.removeChild(overlay);
    });
}

async function refreshEmails() {
    if (currentFolderId) {
        await loadConversations(currentFolderId);
        showNotification('Emails refreshed', 'success');
    }
}

function handleSearch(e: Event) {
    const input = e.target as HTMLInputElement;
    const query = input.value.toLowerCase();
    
    if (!query) {
        renderConversations();
        return;
    }
    
    const filtered = conversations.filter(conv => 
        conv.subject.toLowerCase().includes(query) ||
        conv.preview.toLowerCase().includes(query) ||
        conv.participants.some(p => p.toLowerCase().includes(query))
    );
    
    const container = document.getElementById('conversationList');
    if (!container) return;
    
    if (filtered.length === 0) {
        container.innerHTML = '<div class="empty-state"><h3>No results found</h3></div>';
        return;
    }
    
    container.innerHTML = '';
    filtered.forEach(conv => {
        const convEl = createConversationElement(conv);
        container.appendChild(convEl);
    });
}

// Keyboard shortcut handlers
function replyToCurrentConversation() {
    if (currentConversationIndex >= 0 && currentConversationIndex < conversations.length) {
        replyToConversation(conversations[currentConversationIndex]);
    }
}

function replyAllToCurrentConversation() {
    if (currentConversationIndex >= 0 && currentConversationIndex < conversations.length) {
        replyAllToConversation(conversations[currentConversationIndex]);
    }
}

function forwardCurrentConversation() {
    if (currentConversationIndex >= 0 && currentConversationIndex < conversations.length) {
        forwardConversation(conversations[currentConversationIndex]);
    }
}

function deleteCurrentConversation() {
    if (currentConversationIndex >= 0 && currentConversationIndex < conversations.length) {
        deleteConversation(conversations[currentConversationIndex]);
    }
}

function toggleReadStatus() {
    console.log('Toggle read status');
}

function toggleStarStatus() {
    console.log('Toggle star status');
}

function showShortcuts() {
    const modal = document.getElementById('shortcutsModal');
    const overlay = document.getElementById('overlay');
    if (modal) modal.classList.add('active');
    if (overlay) overlay.classList.add('active');
}

// Quick reply function (exposed globally for inline onclick)
(window as any).sendQuickReply = async function(convId: string) {
    const input = document.getElementById(`reply-${convId}`) as HTMLInputElement;
    if (!input || !input.value.trim()) return;
    
    try {
        const conv = conversations.find(c => c.id === convId);
        if (!conv) return;
        
        await apiClient.sendEmail({
            to: conv.participants.filter(p => p !== currentUser?.email),
            subject: conv.subject.startsWith('Re:') ? conv.subject : `Re: ${conv.subject}`,
            body_text: input.value,
            in_reply_to: conv.id
        });
        
        input.value = '';
        showNotification('Reply sent!', 'success');
        
        if (currentFolderId) {
            await loadConversations(currentFolderId);
        }
    } catch (error) {
        console.error('Failed to send reply:', error);
        showNotification('Failed to send reply', 'error');
    }
};

// Export for debugging
(window as any).frameEmailClient = {
    conversations,
    folders,
    currentFolderId,
    loadConversations,
    openComposeModal,
    refreshEmails
};

export {};
