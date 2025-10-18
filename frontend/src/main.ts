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
import { RichTextEditor } from './components/RichTextEditor';
import { KeyboardShortcuts } from './utils/KeyboardShortcuts';
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
let userSettings: any = {};

// Initialize app
document.addEventListener('DOMContentLoaded', () => {
    initializeApp();
});

async function initializeApp() {
    // Check if user is logged in
    const token = apiClient.getToken();
    if (token) {
        showMainScreen();
        await loadUserSettings();
        await loadInitialData();
        initializeKeyboardShortcuts();
        initializeRichTextEditor();
    } else {
        showLoginScreen();
    }
    
    // Setup event listeners
    setupEventListeners();
    
    // Initialize WebSocket for real-time updates
    initializeWebSocket();
}

function initializeKeyboardShortcuts() {
    const shortcuts = new KeyboardShortcuts();
    
    // Define shortcuts
    shortcuts.register('ctrl+n', () => openComposeModal());
    shortcuts.register('ctrl+r', () => refreshEmails());
    shortcuts.register('ctrl+f', () => document.getElementById('search-input')?.focus());
    shortcuts.register('ctrl+1', () => selectFolderByIndex(0));
    shortcuts.register('ctrl+2', () => selectFolderByIndex(1));
    shortcuts.register('ctrl+3', () => selectFolderByIndex(2));
    shortcuts.register('j', () => navigateConversation('next'));
    shortcuts.register('k', () => navigateConversation('prev'));
    shortcuts.register('o', () => toggleCurrentConversation());
    shortcuts.register('r', () => replyToCurrentConversation());
    shortcuts.register('a', () => replyAllToCurrentConversation());
    shortcuts.register('f', () => forwardCurrentConversation());
    shortcuts.register('delete', () => deleteCurrentConversation());
    shortcuts.register('u', () => toggleReadStatus());
    shortcuts.register('s', () => toggleStarStatus());
    shortcuts.register('/', () => document.getElementById('search-input')?.focus());
    shortcuts.register('g i', () => goToFolder('inbox'));
    shortcuts.register('g s', () => goToFolder('sent'));
    shortcuts.register('g d', () => goToFolder('drafts'));
    shortcuts.register('g t', () => goToFolder('trash'));
    
    if (userSettings.keyboard_shortcuts_enabled) {
        shortcuts.enable();
    }
}

function initializeRichTextEditor() {
    const composeBody = document.getElementById('compose-body') as HTMLTextAreaElement;
    if (composeBody) {
        const editor = new RichTextEditor(composeBody);
        editor.initialize();
    }
}

function initializeWebSocket() {
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const ws = new WebSocket(`${protocol}//${window.location.host}/ws`);
    
    ws.onopen = () => {
        console.log('WebSocket connected');
    };
    
    ws.onmessage = (event) => {
        const data = JSON.parse(event.data);
        handleWebSocketMessage(data);
    };
    
    ws.onerror = (error) => {
        console.error('WebSocket error:', error);
    };
    
    ws.onclose = () => {
        console.log('WebSocket disconnected. Reconnecting in 5 seconds...');
        setTimeout(initializeWebSocket, 5000);
    };
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

async function loadUserSettings() {
    try {
        const settings = await apiClient.getSettings();
        userSettings = settings;
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

function setupEventListeners() {
    // Login form
    const loginForm = document.getElementById('login-form') as HTMLFormElement;
    loginForm?.addEventListener('submit', handleLogin);
    
    // Header buttons
    document.getElementById('menu-toggle')?.addEventListener('click', toggleSidebar);
    document.getElementById('compose-btn')?.addEventListener('click', openComposeModal);
    document.getElementById('refresh-btn')?.addEventListener('click', refreshEmails);
    document.getElementById('logout-btn')?.addEventListener('click', handleLogout);
    
    // Search with debounce
    const searchInput = document.getElementById('search-input') as HTMLInputElement;
    searchInput?.addEventListener('input', debounce(handleSearch, 500));
    
    // Toolbar
    document.getElementById('select-all')?.addEventListener('change', handleSelectAll);
    document.getElementById('mark-read-btn')?.addEventListener('click', () => markSelectedAsRead(true));
    document.getElementById('delete-btn')?.addEventListener('click', deleteSelected);
    document.getElementById('archive-btn')?.addEventListener('click', archiveSelected);
    
    // Compose modal with auto-save
    const composeForm = document.getElementById('compose-form') as HTMLFormElement;
    composeForm?.addEventListener('submit', handleSendEmail);
    composeForm?.addEventListener('input', debounce(autoSaveDraft, 2000));
    
    document.getElementById('create-folder-btn')?.addEventListener('click', createNewFolder);
    
    // Rich text formatting buttons
    document.getElementById('format-bold')?.addEventListener('click', () => formatText('bold'));
    document.getElementById('format-italic')?.addEventListener('click', () => formatText('italic'));
    document.getElementById('format-underline')?.addEventListener('click', () => formatText('underline'));
    document.getElementById('format-link')?.addEventListener('click', () => insertLink());
    document.getElementById('format-list')?.addEventListener('click', () => formatText('insertUnorderedList'));
    document.getElementById('format-ordered-list')?.addEventListener('click', () => formatText('insertOrderedList'));
    
    // File attachment
    document.getElementById('attach-file')?.addEventListener('change', handleFileAttachment);
}

function formatText(command: string, value?: string) {
    const editor = document.getElementById('compose-body-editor');
    if (editor) {
        document.execCommand(command, false, value);
        editor.focus();
    }
}

function insertLink() {
    const url = prompt('Enter URL:');
    if (url) {
        formatText('createLink', url);
    }
}

async function handleFileAttachment(e: Event) {
    const input = e.target as HTMLInputElement;
    const files = input.files;
    
    if (!files || files.length === 0) return;
    
    const maxSize = 25 * 1024 * 1024; // 25MB
    const attachments: any[] = [];
    
    for (const file of Array.from(files)) {
        if (file.size > maxSize) {
            showNotification(`File ${file.name} exceeds 25MB limit`, 'error');
            continue;
        }
        
        const reader = new FileReader();
        reader.onload = (e) => {
            const base64 = (e.target?.result as string).split(',')[1];
            attachments.push({
                filename: file.name,
                content_type: file.type,
                content: base64,
                size: file.size
            });
            
            // Display attachment in UI
            displayAttachment(file.name, file.size);
        };
        reader.readAsDataURL(file);
    }
    
    // Store attachments in draft
    if (currentDraft) {
        currentDraft.attachments = attachments;
    }
}

function displayAttachment(filename: string, size: number) {
    const container = document.getElementById('attachment-list');
    if (!container) return;
    
    const div = document.createElement('div');
    div.className = 'attachment-item';
    div.innerHTML = `
        <span class="attachment-name">📎 ${filename}</span>
        <span class="attachment-size">${formatFileSize(size)}</span>
        <button class="remove-attachment" data-filename="${filename}">×</button>
    `;
    
    container.appendChild(div);
}

function formatFileSize(bytes: number): string {
    if (bytes < 1024) return bytes + ' B';
    if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' KB';
    return (bytes / (1024 * 1024)).toFixed(1) + ' MB';
}

async function autoSaveDraft() {
    if (!currentDraft) {
        currentDraft = {
            id: null,
            to: [],
            subject: '',
            body_text: '',
            body_html: ''
        };
    }
    
    const form = document.getElementById('compose-form') as HTMLFormElement;
    if (!form) return;
    
    const formData = new FormData(form);
    currentDraft.to = (formData.get('to') as string).split(',').map(e => e.trim()).filter(Boolean);
    currentDraft.cc = formData.get('cc') ? (formData.get('cc') as string).split(',').map(e => e.trim()).filter(Boolean) : undefined;
    currentDraft.bcc = formData.get('bcc') ? (formData.get('bcc') as string).split(',').map(e => e.trim()).filter(Boolean) : undefined;
    currentDraft.subject = formData.get('subject') as string;
    
    const editor = document.getElementById('compose-body-editor');
    if (editor) {
        currentDraft.body_html = editor.innerHTML;
        currentDraft.body_text = editor.innerText;
    } else {
        currentDraft.body_text = formData.get('body') as string;
    }
    
    try {
        const response = await apiClient.autoSaveDraft(currentDraft);
        if (!currentDraft.id) {
            currentDraft.id = response.draft_id;
        }
        
        // Show auto-save indicator
        const indicator = document.getElementById('auto-save-indicator');
        if (indicator) {
            indicator.textContent = 'Draft saved';
            indicator.style.opacity = '1';
            setTimeout(() => {
                indicator.style.opacity = '0';
            }, 2000);
        }
    } catch (error) {
        console.error('Failed to auto-save draft:', error);
    }
}

async function toggleConversation(threadId: string) {
    const item = document.querySelector(`[data-thread-id="${threadId}"]`);
    if (!item) return;
    
    item.classList.toggle('expanded');
    
    if (item.classList.contains('expanded')) {
        const messagesContainer = document.getElementById(`messages-${threadId}`)!;
        messagesContainer.innerHTML = '<div class="loading">Loading messages...</div>';
        
        try {
            const emails = await apiClient.getConversation(threadId);
            renderMessages(messagesContainer, emails);
            
            // Auto-mark as read after delay
            if (userSettings.auto_mark_read) {
                const unreadEmails = emails.filter(e => !e.is_read);
                if (unreadEmails.length > 0) {
                    const timer = setTimeout(async () => {
                        await apiClient.markAsRead(unreadEmails.map(e => e.id), true);
                        await loadFolders(); // Update unread counts
                        markAsReadTimers.delete(threadId);
                    }, (userSettings.auto_mark_read_delay || 2) * 1000);
                    
                    markAsReadTimers.set(threadId, timer);
                }
            }
        } catch (error) {
            messagesContainer.innerHTML = '<div class="error">Failed to load messages</div>';
        }
    } else {
        // Cancel mark as read timer if collapsing
        const timer = markAsReadTimers.get(threadId);
        if (timer) {
            clearTimeout(timer);
            markAsReadTimers.delete(threadId);
        }
    }
}

// Keyboard navigation functions
let currentConversationIndex = -1;

function navigateConversation(direction: 'next' | 'prev') {
    const items = document.querySelectorAll('.conversation-item');
    if (items.length === 0) return;
    
    if (direction === 'next') {
        currentConversationIndex = Math.min(currentConversationIndex + 1, items.length - 1);
    } else {
        currentConversationIndex = Math.max(currentConversationIndex - 1, 0);
    }
    
    items.forEach((item, index) => {
        item.classList.toggle('selected', index === currentConversationIndex);
    });
    
    items[currentConversationIndex]?.scrollIntoView({ behavior: 'smooth', block: 'center' });
}

function toggleCurrentConversation() {
    const items = document.querySelectorAll('.conversation-item');
    if (currentConversationIndex >= 0 && currentConversationIndex < items.length) {
        const item = items[currentConversationIndex] as HTMLElement;
        const threadId = item.dataset.threadId;
        if (threadId) {
            toggleConversation(threadId);
        }
    }
}

function selectFolderByIndex(index: number) {
    const folderItems = document.querySelectorAll('.folder-item');
    if (index < folderItems.length) {
        (folderItems[index] as HTMLElement).click();
    }
}

function goToFolder(folderType: string) {
    const folder = folders.find(f => f.folder_type === folderType);
    if (folder) {
        selectFolder(folder.id);
    }
}

// Continue with remaining functions...
// [Previous implementation continues with improvements]

export {};