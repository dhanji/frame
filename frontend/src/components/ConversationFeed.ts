import { EmailAPI } from '../api/client';
import { Conversation, Email } from '../models/types';
import { formatDate, sanitizeHtml } from '../utils/helpers';
import { InlineReply } from './InlineReply';
import DOMPurify from 'dompurify';

export class ConversationFeed {
    private container: HTMLElement;
    private api: EmailAPI;
    private conversations: Conversation[] = [];
    private currentFolder: string = 'INBOX';
    private selectedConversations: Set<string> = new Set();
    private inlineReplies: Map<string, InlineReply> = new Map();
    private expandedConversations: Set<string> = new Set();
    private csrfToken: string = '';

    constructor(container: HTMLElement, api: EmailAPI) {
        this.container = container;
        this.api = api;
        this.initialize();
    }

    private async initialize() {
        // Get CSRF token
        this.csrfToken = await this.api.getCsrfToken();
        
        // Set up event listeners
        this.setupEventListeners();
        
        // Load initial conversations
        await this.loadConversations();
        
        // Set up auto-refresh
        setInterval(() => this.refreshConversations(), 30000); // Refresh every 30 seconds
    }

    private setupEventListeners() {
        // Select all checkbox
        const selectAll = document.getElementById('selectAll') as HTMLInputElement;
        if (selectAll) {
            selectAll.addEventListener('change', (e) => {
                const checked = (e.target as HTMLInputElement).checked;
                this.selectAllConversations(checked);
            });
        }

        // Bulk action buttons
        document.getElementById('archiveBtn')?.addEventListener('click', () => {
            this.bulkAction('archive');
        });
        
        document.getElementById('deleteBtn')?.addEventListener('click', () => {
            this.bulkAction('delete');
        });
        
        document.getElementById('markReadBtn')?.addEventListener('click', () => {
            this.bulkAction('mark_read');
        });

        // Folder navigation
        document.querySelectorAll('.folder-item').forEach(item => {
            item.addEventListener('click', (e) => {
                const folder = (e.currentTarget as HTMLElement).dataset.folder;
                if (folder) {
                    this.switchFolder(folder);
                }
            });
        });
    }

    public async loadConversations() {
        this.showLoading();
        
        try {
            const response = await this.api.getConversations(this.currentFolder);
            this.conversations = response.data;
            this.render();
        } catch (error) {
            console.error('Failed to load conversations:', error);
            this.showError('Failed to load conversations. Please try again.');
        }
    }

    private async refreshConversations() {
        try {
            const response = await this.api.getConversations(this.currentFolder);
            this.conversations = response.data;
            this.render();
        } catch (error) {
            console.error('Failed to refresh conversations:', error);
        }
    }

    private render() {
        this.container.innerHTML = '';
        
        if (this.conversations.length === 0) {
            this.container.innerHTML = `
                <div class="empty-state">
                    <i class="fas fa-inbox fa-3x"></i>
                    <p>No conversations in ${this.currentFolder}</p>
                </div>
            `;
            return;
        }

        this.conversations.forEach(conversation => {
            const conversationEl = this.createConversationElement(conversation);
            this.container.appendChild(conversationEl);
        });
    }

    private createConversationElement(conversation: Conversation): HTMLElement {
        const div = document.createElement('div');
        div.className = `conversation-thread ${conversation.unread_count > 0 ? 'unread' : ''} ${this.expandedConversations.has(conversation.id) ? 'expanded' : ''}`;
        div.dataset.conversationId = conversation.id;
        
        // Create conversation header
        const header = document.createElement('div');
        header.className = 'conversation-header';
        header.innerHTML = `
            <div class="conversation-select">
                <input type="checkbox" class="conversation-checkbox" 
                       data-id="${conversation.id}" 
                       ${this.selectedConversations.has(conversation.id) ? 'checked' : ''}>
            </div>
            <div class="conversation-content" data-id="${conversation.id}">
                <div class="conversation-meta">
                    <div class="conversation-participants">
                        <div class="participant-avatar">
                            ${this.getAvatarInitials(conversation.participants[0])}
                        </div>
                        <div class="participant-info">
                            <div class="participant-name">
                                ${this.formatParticipants(conversation.participants)}
                            </div>
                            ${conversation.participants.length > 1 ? 
                                `<div class="participant-count">${conversation.participants.length} participants</div>` : ''}
                        </div>
                    </div>
                    <div class="conversation-date">
                        ${formatDate(conversation.last_message_date)}
                    </div>
                </div>
                <div class="conversation-subject">
                    ${DOMPurify.sanitize(conversation.subject)}
                    ${conversation.unread_count > 0 ? 
                        `<span class="unread-indicator">${conversation.unread_count} new</span>` : ''}
                </div>
                <div class="conversation-preview">
                    ${this.renderPreviewMessages(conversation.preview_messages)}
                </div>
                <div class="conversation-actions">
                    ${conversation.has_attachments ? '<i class="fas fa-paperclip"></i>' : ''}
                    ${conversation.is_starred ? '<i class="fas fa-star starred"></i>' : ''}
                    <button class="conversation-action" data-action="reply" data-id="${conversation.id}">
                        <i class="fas fa-reply"></i> Reply
                    </button>
                    <button class="conversation-action" data-action="reply-all" data-id="${conversation.id}">
                        <i class="fas fa-reply-all"></i> Reply All
                    </button>
                    <button class="conversation-action" data-action="forward" data-id="${conversation.id}">
                        <i class="fas fa-share"></i> Forward
                    </button>
                </div>
            </div>
        `;
        
        // Add click handler to expand conversation
        header.querySelector('.conversation-content')?.addEventListener('click', (e) => {
            if (!(e.target as HTMLElement).closest('.conversation-action')) {
                this.toggleConversation(conversation.id);
            }
        });
        
        // Add checkbox handler
        const checkbox = header.querySelector('.conversation-checkbox') as HTMLInputElement;
        checkbox?.addEventListener('change', (e) => {
            const checked = (e.target as HTMLInputElement).checked;
            if (checked) {
                this.selectedConversations.add(conversation.id);
            } else {
                this.selectedConversations.delete(conversation.id);
            }
            this.updateBulkActionButtons();
        });
        
        // Add action handlers
        header.querySelectorAll('.conversation-action').forEach(btn => {
            btn.addEventListener('click', (e) => {
                e.stopPropagation();
                const action = (e.currentTarget as HTMLElement).dataset.action;
                if (action) {
                    this.handleConversationAction(action, conversation);
                }
            });
        });
        
        div.appendChild(header);
        
        // Create expanded messages container
        if (this.expandedConversations.has(conversation.id)) {
            const messagesContainer = document.createElement('div');
            messagesContainer.className = 'conversation-messages';
            messagesContainer.innerHTML = '<div class="loading-messages">Loading messages...</div>';
            div.appendChild(messagesContainer);
            
            // Load full conversation
            this.loadFullConversation(conversation.id, messagesContainer);
        }
        
        return div;
    }

    private renderPreviewMessages(messages: any[]): string {
        return messages.map(msg => `
            <div class="preview-message ${!msg.is_read ? 'unread' : ''}">
                <span class="preview-from">${DOMPurify.sanitize(msg.from)}:</span>
                <span class="preview-text">${DOMPurify.sanitize(msg.preview)}</span>
            </div>
        `).join('');
    }

    private async toggleConversation(conversationId: string) {
        if (this.expandedConversations.has(conversationId)) {
            this.expandedConversations.delete(conversationId);
        } else {
            this.expandedConversations.add(conversationId);
        }
        this.render();
    }

    private async loadFullConversation(conversationId: string, container: HTMLElement) {
        try {
            const conversation = await this.api.getConversation(conversationId);
            
            // Render messages
            const messagesHtml = conversation.messages.map(msg => this.renderMessage(msg)).join('');
            
            // Add inline reply at the bottom
            const inlineReplyHtml = `
                <div class="inline-reply" id="inline-reply-${conversationId}">
                    <div class="reply-composer">
                        <textarea class="reply-input" placeholder="Type your reply..."></textarea>
                        <div class="reply-actions">
                            <div class="reply-options">
                                <button class="reply-option-btn" data-type="attachment">
                                    <i class="fas fa-paperclip"></i>
                                </button>
                                <button class="reply-option-btn" data-type="emoji">
                                    <i class="fas fa-smile"></i>
                                </button>
                            </div>
                            <div class="reply-buttons">
                                <button class="reply-btn secondary" data-action="save-draft">
                                    Save Draft
                                </button>
                                <button class="reply-btn primary" data-action="send">
                                    <i class="fas fa-paper-plane"></i> Send
                                </button>
                            </div>
                        </div>
                    </div>
                </div>
            `;
            
            container.innerHTML = messagesHtml + inlineReplyHtml;
            
            // Initialize inline reply
            const replyContainer = container.querySelector(`#inline-reply-${conversationId}`);
            if (replyContainer) {
                const inlineReply = new InlineReply(
                    replyContainer as HTMLElement,
                    this.api,
                    conversationId,
                    conversation
                );
                this.inlineReplies.set(conversationId, inlineReply);
            }
            
            // Mark as read after 2 seconds
            setTimeout(() => {
                this.api.markAsRead(conversationId);
            }, 2000);
            
        } catch (error) {
            console.error('Failed to load conversation:', error);
            container.innerHTML = '<div class="error-message">Failed to load messages</div>';
        }
    }

    private renderMessage(email: Email): string {
        const sanitizedBody = email.body_html ? 
            DOMPurify.sanitize(email.body_html) : 
            DOMPurify.sanitize(email.body_text || '').replace(/\n/g, '<br>');
        
        return `
            <div class="email-message" data-id="${email.id}">
                <div class="message-header">
                    <div class="message-sender">
                        <div class="sender-avatar">
                            ${this.getAvatarInitials(email.from)}
                        </div>
                        <div class="sender-info">
                            <div class="sender-name">${DOMPurify.sanitize(email.from)}</div>
                            <div class="sender-email">to ${DOMPurify.sanitize(email.to.join(', '))}</div>
                        </div>
                    </div>
                    <div class="message-date">${formatDate(email.date)}</div>
                </div>
                <div class="message-body">
                    ${sanitizedBody}
                </div>
                ${email.attachments && email.attachments.length > 0 ? `
                    <div class="message-attachments">
                        ${email.attachments.map(att => `
                            <div class="attachment-item">
                                <i class="fas fa-file attachment-icon"></i>
                                <span class="attachment-name">${DOMPurify.sanitize(att.filename)}</span>
                                <span class="attachment-size">${this.formatFileSize(att.size)}</span>
                            </div>
                        `).join('')}
                    </div>
                ` : ''}
            </div>
        `;
    }

    private async bulkAction(action: string) {
        if (this.selectedConversations.size === 0) {
            alert('Please select conversations first');
            return;
        }
        
        try {
            await this.api.bulkAction({
                conversation_ids: Array.from(this.selectedConversations),
                action,
                csrf_token: this.csrfToken
            });
            
            // Clear selection and reload
            this.selectedConversations.clear();
            await this.loadConversations();
        } catch (error) {
            console.error('Bulk action failed:', error);
            alert('Failed to perform bulk action');
        }
    }

    private selectAllConversations(selected: boolean) {
        if (selected) {
            this.conversations.forEach(conv => {
                this.selectedConversations.add(conv.id);
            });
        } else {
            this.selectedConversations.clear();
        }
        this.render();
        this.updateBulkActionButtons();
    }

    private updateBulkActionButtons() {
        const hasSelection = this.selectedConversations.size > 0;
        document.querySelectorAll('.toolbar-btn').forEach(btn => {
            (btn as HTMLButtonElement).disabled = !hasSelection;
        });
    }

    private async handleConversationAction(action: string, conversation: Conversation) {
        switch (action) {
            case 'reply':
            case 'reply-all':
            case 'forward':
                // Expand conversation and focus on inline reply
                if (!this.expandedConversations.has(conversation.id)) {
                    this.expandedConversations.add(conversation.id);
                    this.render();
                }
                // Focus will be set by InlineReply component
                break;
        }
    }

    private async switchFolder(folder: string) {
        this.currentFolder = folder;
        
        // Update UI
        document.querySelectorAll('.folder-item').forEach(item => {
            item.classList.toggle('active', item.getAttribute('data-folder') === folder);
        });
        
        // Load conversations for new folder
        await this.loadConversations();
    }

    private getAvatarInitials(email: string): string {
        const name = email.split('<')[0].trim();
        const parts = name.split(' ');
        if (parts.length >= 2) {
            return parts[0][0] + parts[parts.length - 1][0];
        }
        return name.substring(0, 2).toUpperCase();
    }

    private formatParticipants(participants: string[]): string {
        if (participants.length === 1) {
            return participants[0];
        } else if (participants.length === 2) {
            return participants.join(', ');
        } else {
            return `${participants[0]}, ${participants[1]} +${participants.length - 2} more`;
        }
    }

    private formatFileSize(bytes: number): string {
        if (bytes < 1024) return bytes + ' B';
        if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' KB';
        return (bytes / (1024 * 1024)).toFixed(1) + ' MB';
    }

    private showLoading() {
        this.container.innerHTML = `
            <div class="loading-spinner">
                <i class="fas fa-spinner fa-spin"></i>
                <p>Loading conversations...</p>
            </div>
        `;
    }

    private showError(message: string) {
        this.container.innerHTML = `
            <div class="error-state">
                <i class="fas fa-exclamation-triangle fa-3x"></i>
                <p>${message}</p>
                <button onclick="location.reload()">Retry</button>
            </div>
        `;
    }
}