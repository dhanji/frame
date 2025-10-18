// API configuration
const API_BASE = 'http://localhost:8080/api';
let authToken = localStorage.getItem('authToken');
let currentFolder = 'inbox';
let selectedConversation = null;
let currentFilter = 'all';
let wsConnection = null;

// Mock data for demonstration
const mockConversations = [
    {
        id: '1',
        subject: 'Project Update - Q4 Planning',
        participants: ['John Doe', 'Jane Smith'],
        lastMessageDate: '2 hours ago',
        unread: true,
        starred: false,
        hasAttachments: true,
        messages: [
            { from: 'John Doe', text: 'Hi team, I wanted to share the latest updates on our Q4 planning...', time: '2 hours ago' },
            { from: 'Jane Smith', text: 'Thanks John! I have a few questions about the timeline...', time: '1 hour ago' }
        ]
    },
    {
        id: '2',
        subject: 'Meeting Tomorrow',
        participants: ['Alice Johnson'],
        lastMessageDate: '5 hours ago',
        unread: false,
        starred: true,
        hasAttachments: false,
        messages: [
            { from: 'Alice Johnson', text: 'Just a reminder about our meeting tomorrow at 2 PM...', time: '5 hours ago' }
        ]
    },
    {
        id: '3',
        subject: 'Weekly Newsletter',
        participants: ['Newsletter Team'],
        lastMessageDate: '1 day ago',
        unread: false,
        starred: false,
        hasAttachments: false,
        messages: [
            { from: 'Newsletter Team', text: 'This week in tech: AI advances, new frameworks, and more...', time: '1 day ago' }
        ]
    }
];

// Initialize WebSocket connection
function initWebSocket() {
    if (!authToken) return;
    
    const wsUrl = `ws://localhost:8080/ws`;
    wsConnection = new WebSocket(wsUrl);
    
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
        console.log('WebSocket disconnected');
        // Attempt to reconnect after 5 seconds
        setTimeout(initWebSocket, 5000);
    };
}

// Handle WebSocket messages
function handleWebSocketMessage(message) {
    switch(message.type) {
        case 'NewEmail':
            showNotification('New Email', `From: ${message.from}\n${message.subject}`);
            loadConversations();
            break;
        case 'EmailRead':
            updateEmailStatus(message.email_id, 'read');
            break;
        case 'EmailDeleted':
            removeEmailFromView(message.email_id);
            break;
        case 'FolderUpdate':
            updateFolderCount(message.folder, message.unread_count);
            break;
        case 'Ping':
            if (wsConnection) {
                wsConnection.send(JSON.stringify({ type: 'Pong' }));
            }
            break;
    }
}

// Load conversations from API or mock data
async function loadConversations() {
    const container = document.getElementById('conversationList');
    
    if (authToken) {
        try {
            const response = await fetch(`${API_BASE}/conversations?folder=${currentFolder}`, {
                headers: {
                    'Authorization': `Bearer ${authToken}`
                }
            });
            
            if (response.ok) {
                const conversations = await response.json();
                displayConversations(conversations);
            } else {
                // Fall back to mock data
                displayConversations(filterConversations(mockConversations));
            }
        } catch (error) {
            console.error('Error loading conversations:', error);
            displayConversations(filterConversations(mockConversations));
        }
    } else {
        displayConversations(filterConversations(mockConversations));
    }
}

// Filter conversations based on current filter
function filterConversations(conversations) {
    switch(currentFilter) {
        case 'unread':
            return conversations.filter(c => c.unread);
        case 'starred':
            return conversations.filter(c => c.starred);
        case 'attachments':
            return conversations.filter(c => c.hasAttachments);
        default:
            return conversations;
    }
}

// Display conversations in the UI
function displayConversations(conversations) {
    const container = document.getElementById('conversationList');
    container.innerHTML = '';
    
    conversations.forEach(conv => {
        const convEl = createConversationElement(conv);
        container.appendChild(convEl);
    });
}

// Create conversation element
function createConversationElement(conv) {
    const div = document.createElement('div');
    div.className = `conversation ${conv.unread ? 'unread' : ''}`;
    div.dataset.id = conv.id;
    div.innerHTML = `
        <div class="conversation-header">
            <div class="participants">${conv.participants.join(', ')}</div>
            <div class="timestamp">${conv.lastMessageDate}</div>
        </div>
        <div class="subject">
            ${conv.starred ? '⭐ ' : ''}${conv.subject}
            ${conv.hasAttachments ? '📎' : ''}
        </div>
        ${conv.messages.map(msg => `
            <div class="message-preview">
                <div class="message-from">${msg.from}</div>
                <div class="message-text">${msg.text}</div>
            </div>
        `).join('')}
        <div class="quick-reply">
            <input type="text" placeholder="Write a reply..." id="reply-${conv.id}">
            <button onclick="sendReply('${conv.id}')">Send</button>
        </div>
        <div class="conversation-actions">
            <button class="action-btn" onclick="replyTo('${conv.id}')">Reply</button>
            <button class="action-btn" onclick="replyAll('${conv.id}')">Reply All</button>
            <button class="action-btn" onclick="forward('${conv.id}')">Forward</button>
            <button class="action-btn" onclick="deleteEmail('${conv.id}')">Delete</button>
            <button class="action-btn" onclick="toggleStar('${conv.id}')">${conv.starred ? 'Unstar' : 'Star'}</button>
            <button class="action-btn" onclick="toggleRead('${conv.id}')">${conv.unread ? 'Mark Read' : 'Mark Unread'}</button>
        </div>
    `;
    
    // Mark as read after 2 seconds of viewing
    if (conv.unread) {
        const observer = new IntersectionObserver((entries) => {
            entries.forEach(entry => {
                if (entry.isIntersecting) {
                    setTimeout(() => {
                        if (entry.isIntersecting) {
                            markAsRead(conv.id);
                        }
                    }, 2000);
                }
            });
        });
        observer.observe(div);
    }
    
    return div;
}

// Apply filter
function applyFilter(filter) {
    currentFilter = filter;
    
    // Update UI
    document.querySelectorAll('.filter-chip').forEach(chip => {
        chip.classList.remove('active');
    });
    document.querySelector(`[data-filter="${filter}"]`).classList.add('active');
    
    // Reload conversations
    loadConversations();
}

// Toggle advanced search
function toggleAdvancedSearch() {
    const panel = document.getElementById('advancedSearch');
    panel.classList.toggle('active');
}

// Perform advanced search
async function performAdvancedSearch() {
    const searchParams = {
        text: document.getElementById('searchInput').value,
        from: document.getElementById('searchFrom').value,
        to: document.getElementById('searchTo').value,
        dateFrom: document.getElementById('searchDateFrom').value,
        dateTo: document.getElementById('searchDateTo').value,
        hasAttachments: document.getElementById('searchHasAttachment').value
    };
    
    // Filter out empty values
    const query = Object.entries(searchParams)
        .filter(([_, value]) => value)
        .map(([key, value]) => `${key}=${encodeURIComponent(value)}`)
        .join('&');
    
    if (authToken) {
        try {
            const response = await fetch(`${API_BASE}/search?${query}`, {
                headers: {
                    'Authorization': `Bearer ${authToken}`
                }
            });
            
            if (response.ok) {
                const results = await response.json();
                displayConversations(results);
            }
        } catch (error) {
            console.error('Search error:', error);
        }
    }
}

// Compose functions
function openComposeModal() {
    document.getElementById('composeModal').classList.add('active');
    document.getElementById('overlay').classList.add('active');
}

function closeComposeModal() {
    document.getElementById('composeModal').classList.remove('active');
    document.getElementById('overlay').classList.remove('active');
}

async function sendEmail() {
    const emailData = {
        to: document.getElementById('composeTo').value.split(',').map(e => e.trim()),
        cc: document.getElementById('composeCc').value.split(',').map(e => e.trim()).filter(e => e),
        bcc: [],
        subject: document.getElementById('composeSubject').value,
        body_text: document.getElementById('composeBody').value,
        body_html: null,
        attachments: []
    };
    
    if (authToken) {
        try {
            const response = await fetch(`${API_BASE}/emails/send`, {
                method: 'POST',
                headers: {
                    'Authorization': `Bearer ${authToken}`,
                    'Content-Type': 'application/json'
                },
                body: JSON.stringify(emailData)
            });
            
            if (response.ok) {
                alert('Email sent successfully!');
                closeComposeModal();
                loadConversations();
            } else {
                alert('Failed to send email');
            }
        } catch (error) {
            console.error('Error sending email:', error);
            alert('Error sending email');
        }
    } else {
        alert('Email would be sent: ' + JSON.stringify(emailData));
        closeComposeModal();
    }
}

function saveDraft() {
    const draftData = {
        to: document.getElementById('composeTo').value,
        cc: document.getElementById('composeCc').value,
        subject: document.getElementById('composeSubject').value,
        body: document.getElementById('composeBody').value
    };
    
    localStorage.setItem('draft', JSON.stringify(draftData));
    alert('Draft saved!');
}

// Settings functions
function toggleSettings() {
    const settingsPanel = document.getElementById('settingsPanel');
    const conversationList = document.getElementById('conversationList');
    
    if (settingsPanel.style.display === 'none') {
        settingsPanel.style.display = 'block';
        conversationList.style.display = 'none';
    } else {
        settingsPanel.style.display = 'none';
        conversationList.style.display = 'block';
    }
}

function saveSettings() {
    // Save settings to localStorage or API
    const settings = {
        notifications: document.querySelector('input[type="checkbox"]').checked,
        autoMarkRead: document.querySelectorAll('input[type="checkbox"]')[1].checked,
        conversationView: document.querySelectorAll('input[type="checkbox"]')[2].checked
    };
    
    localStorage.setItem('settings', JSON.stringify(settings));
    alert('Settings saved!');
}

// Folder functions
function createFolder() {
    const folderName = prompt('Enter folder name:');
    if (folderName) {
        const customFolders = document.getElementById('customFolders');
        const folderDiv = document.createElement('div');
        folderDiv.className = 'folder-item';
        folderDiv.dataset.folder = folderName.toLowerCase();
        folderDiv.innerHTML = `
            <span><span class="folder-icon">📁</span>${folderName}</span>
        `;
        folderDiv.addEventListener('click', () => selectFolder(folderName.toLowerCase()));
        customFolders.appendChild(folderDiv);
    }
}

function selectFolder(folder) {
    currentFolder = folder;
    
    // Update UI
    document.querySelectorAll('.folder-item').forEach(item => {
        item.classList.remove('active');
    });
    document.querySelector(`[data-folder="${folder}"]`).classList.add('active');
    
    // Load conversations for this folder
    loadConversations();
}

// Email action functions
async function sendReply(convId) {
    const input = document.getElementById(`reply-${convId}`);
    if (input.value.trim()) {
        if (authToken) {
            try {
                const response = await fetch(`${API_BASE}/emails/${convId}/reply`, {
                    method: 'POST',
                    headers: {
                        'Authorization': `Bearer ${authToken}`,
                        'Content-Type': 'application/json'
                    },
                    body: JSON.stringify({
                        conversation_id: convId,
                        reply_type: 'reply',
                        to: [],
                        cc: [],
                        bcc: [],
                        subject: 'Re: ',
                        body_text: input.value,
                        body_html: null,
                        attachments: []
                    })
                });
                
                if (response.ok) {
                    input.value = '';
                    loadConversations();
                }
            } catch (error) {
                console.error('Error sending reply:', error);
            }
        } else {
            alert(`Reply sent: ${input.value}`);
            input.value = '';
        }
    }
}

function replyTo(convId) {
    selectedConversation = convId;
    document.getElementById(`reply-${convId}`)?.focus();
}

function replyAll(convId) {
    selectedConversation = convId;
    alert(`Reply all to conversation ${convId}`);
}

function forward(convId) {
    selectedConversation = convId;
    alert(`Forward conversation ${convId}`);
}

async function deleteEmail(convId) {
    if (confirm('Move this conversation to trash?')) {
        if (authToken) {
            try {
                const response = await fetch(`${API_BASE}/emails/${convId}`, {
                    method: 'DELETE',
                    headers: {
                        'Authorization': `Bearer ${authToken}`
                    }
                });
                
                if (response.ok) {
                    loadConversations();
                }
            } catch (error) {
                console.error('Error deleting email:', error);
            }
        } else {
            alert(`Deleted conversation ${convId}`);
            loadConversations();
        }
    }
}

function toggleStar(convId) {
    const conv = mockConversations.find(c => c.id === convId);
    if (conv) {
        conv.starred = !conv.starred;
        loadConversations();
    }
}

function toggleRead(convId) {
    const conv = mockConversations.find(c => c.id === convId);
    if (conv) {
        conv.unread = !conv.unread;
        loadConversations();
    }
}

async function markAsRead(convId) {
    if (authToken) {
        try {
            await fetch(`${API_BASE}/emails/${convId}/read`, {
                method: 'PUT',
                headers: {
                    'Authorization': `Bearer ${authToken}`,
                    'Content-Type': 'application/json'
                },
                body: JSON.stringify({ is_read: true })
            });
        } catch (error) {
            console.error('Error marking as read:', error);
        }
    }
    
    const conv = mockConversations.find(c => c.id === convId);
    if (conv) {
        conv.unread = false;
        loadConversations();
    }
}

// Keyboard shortcuts
document.addEventListener('keydown', (e) => {
    // Don't trigger shortcuts when typing in input fields
    if (e.target.tagName === 'INPUT' || e.target.tagName === 'TEXTAREA') {
        return;
    }
    
    switch(e.key) {
        case 'c':
        case 'C':
            openComposeModal();
            break;
        case '/':
            e.preventDefault();
            document.getElementById('searchInput').focus();
            break;
        case 'r':
        case 'R':
            if (selectedConversation) replyTo(selectedConversation);
            break;
        case 'a':
        case 'A':
            if (selectedConversation) replyAll(selectedConversation);
            break;
        case 'f':
        case 'F':
            if (selectedConversation) forward(selectedConversation);
            break;
        case 'd':
        case 'D':
            if (selectedConversation) deleteEmail(selectedConversation);
            break;
        case 's':
        case 'S':
            if (selectedConversation) toggleStar(selectedConversation);
            break;
        case 'u':
        case 'U':
            if (selectedConversation) toggleRead(selectedConversation);
            break;
        case '?':
            showShortcuts();
            break;
    }
});

function showShortcuts() {
    document.getElementById('shortcutsModal').classList.add('active');
    document.getElementById('overlay').classList.add('active');
}

function closeShortcuts() {
    document.getElementById('shortcutsModal').classList.remove('active');
    document.getElementById('overlay').classList.remove('active');
}

function closeModals() {
    document.querySelectorAll('.modal').forEach(modal => {
        modal.classList.remove('active');
    });
    document.getElementById('overlay').classList.remove('active');
}

// Notification helper
function showNotification(title, body) {
    if ('Notification' in window && Notification.permission === 'granted') {
        new Notification(title, { body });
    }
}

// Request notification permission
if ('Notification' in window && Notification.permission === 'default') {
    Notification.requestPermission();
}

// Folder navigation
document.querySelectorAll('.folder-item').forEach(item => {
    item.addEventListener('click', () => {
        selectFolder(item.dataset.folder);
    });
});

// Search on enter
document.getElementById('searchInput').addEventListener('keypress', (e) => {
    if (e.key === 'Enter') {
        performAdvancedSearch();
    }
});

// Initialize
loadConversations();
initWebSocket();

function initWebSocket() {
    try {
        wsConnection = new WebSocket('ws://localhost:8080/ws');
        wsConnection.onopen = () => console.log('WebSocket connected');
        wsConnection.onmessage = (event) => handleWebSocketMessage(event);
        wsConnection.onclose = () => console.log('WebSocket disconnected');
    } catch (error) {
        console.error('WebSocket connection failed:', error);
    }
}

function handleWebSocketMessage(event) {
    try {
        const message = JSON.parse(event.data);
        switch (message.type) {
            case 'NewEmail':
                showNotification('New Email', `From: ${message.from} - ${message.subject}`);
                loadConversations(); // Refresh conversation list
                break;
            case 'Pong':
                // Handle pong response
                break;
        }
    } catch (error) {
        console.error('Error handling WebSocket message:', error);
    }
}

// Load saved draft if exists
const savedDraft = localStorage.getItem('draft');
if (savedDraft) {
    const draft = JSON.parse(savedDraft);
    // Populate compose form if needed
}

// Load settings
const savedSettings = localStorage.getItem('settings');
if (savedSettings) {
    const settings = JSON.parse(savedSettings);
    // Apply settings
}
