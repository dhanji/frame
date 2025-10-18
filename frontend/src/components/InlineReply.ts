import { EmailAPI } from '../api/client';
import { Conversation, Email } from '../models/types';
import DOMPurify from 'dompurify';

export class InlineReply {
    private container: HTMLElement;
    private api: EmailAPI;
    private conversationId: string;
    private conversation: Conversation;
    private replyType: 'reply' | 'reply-all' | 'forward' = 'reply';
    private draftId: string | null = null;
    private autoSaveTimer: NodeJS.Timeout | null = null;
    private textarea: HTMLTextAreaElement;
    private attachments: File[] = [];

    constructor(
        container: HTMLElement,
        api: EmailAPI,
        conversationId: string,
        conversation: Conversation
    ) {
        this.container = container;
        this.api = api;
        this.conversationId = conversationId;
        this.conversation = conversation;
        this.textarea = container.querySelector('.reply-input') as HTMLTextAreaElement;
        this.initialize();
    }

    private initialize() {
        this.setupEventListeners();
        this.setupAutoSave();
        this.loadDraft();
    }

    private setupEventListeners() {
        // Send button
        const sendBtn = this.container.querySelector('[data-action="send"]');
        sendBtn?.addEventListener('click', () => this.sendReply());

        // Save draft button
        const saveDraftBtn = this.container.querySelector('[data-action="save-draft"]');
        saveDraftBtn?.addEventListener('click', () => this.saveDraft());

        // Attachment button
        const attachBtn = this.container.querySelector('[data-type="attachment"]');
        attachBtn?.addEventListener('click', () => this.selectAttachments());

        // Reply type selector
        const replyTypeButtons = this.container.querySelectorAll('.reply-type-btn');
        replyTypeButtons.forEach(btn => {
            btn.addEventListener('click', (e) => {
                const type = (e.currentTarget as HTMLElement).dataset.replyType as 'reply' | 'reply-all' | 'forward';
                this.setReplyType(type);
            });
        });

        // Auto-resize textarea
        this.textarea.addEventListener('input', () => {
            this.autoResize();
            this.triggerAutoSave();
        });

        // Keyboard shortcuts
        this.textarea.addEventListener('keydown', (e) => {
            if (e.ctrlKey || e.metaKey) {
                if (e.key === 'Enter') {
                    e.preventDefault();
                    this.sendReply();
                } else if (e.key === 's') {
                    e.preventDefault();
                    this.saveDraft();
                }
            }
        });
    }

    private setupAutoSave() {
        // Auto-save draft every 10 seconds when typing
        this.textarea.addEventListener('input', () => {
            this.triggerAutoSave();
        });
    }

    private triggerAutoSave() {
        if (this.autoSaveTimer) {
            clearTimeout(this.autoSaveTimer);
        }
        
        this.autoSaveTimer = setTimeout(() => {
            this.saveDraft(true); // Silent save
        }, 10000); // 10 seconds
    }

    private async loadDraft() {
        try {
            const drafts = await this.api.getDrafts();
            const draft = drafts.find(d => d.in_reply_to === this.conversationId);
            
            if (draft) {
                this.draftId = draft.id;
                this.textarea.value = draft.body_text || '';
                this.autoResize();
                this.showNotification('Draft loaded', 'info');
            }
        } catch (error) {
            console.error('Failed to load draft:', error);
        }
    }

    private async saveDraft(silent: boolean = false) {
        const content = this.textarea.value.trim();
        
        if (!content && !this.attachments.length) {
            if (!silent) {
                this.showNotification('Nothing to save', 'warning');
            }
            return;
        }

        try {
            const draftData = {
                id: this.draftId,
                in_reply_to: this.conversationId,
                to: this.getRecipients(),
                subject: this.getSubject(),
                body_text: content,
                body_html: this.convertToHtml(content),
                attachments: this.attachments
            };

            const response = await this.api.saveDraft(draftData);
            this.draftId = response.id;
            
            if (!silent) {
                this.showNotification('Draft saved', 'success');
            }
        } catch (error) {
            console.error('Failed to save draft:', error);
            if (!silent) {
                this.showNotification('Failed to save draft', 'error');
            }
        }
    }

    private async sendReply() {
        const content = this.textarea.value.trim();
        
        if (!content && !this.attachments.length) {
            this.showNotification('Please enter a message', 'warning');
            return;
        }

        // Show sending state
        const sendBtn = this.container.querySelector('[data-action="send"]') as HTMLButtonElement;
        const originalText = sendBtn.innerHTML;
        sendBtn.disabled = true;
        sendBtn.innerHTML = '<i class="fas fa-spinner fa-spin"></i> Sending...';

        try {
            const emailData = {
                conversation_id: this.conversationId,
                type: this.replyType,
                to: this.getRecipients(),
                subject: this.getSubject(),
                body_text: content,
                body_html: this.convertToHtml(content),
                attachments: await this.uploadAttachments(),
                in_reply_to: this.conversation.messages[0].message_id,
                references: this.conversation.messages.map(m => m.message_id)
            };

            await this.api.sendReply(emailData);
            
            // Clear the form
            this.textarea.value = '';
            this.attachments = [];
            this.updateAttachmentsList();
            
            // Delete draft if exists
            if (this.draftId) {
                await this.api.deleteDraft(this.draftId);
                this.draftId = null;
            }
            
            this.showNotification('Reply sent successfully', 'success');
            
            // Reload conversation to show new message
            setTimeout(() => {
                window.location.reload();
            }, 1000);
            
        } catch (error) {
            console.error('Failed to send reply:', error);
            this.showNotification('Failed to send reply', 'error');
        } finally {
            sendBtn.disabled = false;
            sendBtn.innerHTML = originalText;
        }
    }

    private setReplyType(type: 'reply' | 'reply-all' | 'forward') {
        this.replyType = type;
        
        // Update UI to show selected type
        this.container.querySelectorAll('.reply-type-btn').forEach(btn => {
            btn.classList.toggle('active', btn.getAttribute('data-reply-type') === type);
        });
        
        // Update placeholder text
        switch (type) {
            case 'reply':
                this.textarea.placeholder = 'Type your reply...';
                break;
            case 'reply-all':
                this.textarea.placeholder = 'Type your reply to all...';
                break;
            case 'forward':
                this.textarea.placeholder = 'Add a message to forward...';
                break;
        }
    }

    private getRecipients(): string[] {
        const lastMessage = this.conversation.messages[this.conversation.messages.length - 1];
        
        switch (this.replyType) {
            case 'reply':
                return [lastMessage.from];
            case 'reply-all':
                const recipients = [lastMessage.from, ...lastMessage.to, ...lastMessage.cc];
                // Remove current user's email
                return [...new Set(recipients)].filter(email => !email.includes('@me'));
            case 'forward':
                // For forward, recipients will be entered separately
                return [];
            default:
                return [lastMessage.from];
        }
    }

    private getSubject(): string {
        const originalSubject = this.conversation.subject;
        
        switch (this.replyType) {
            case 'reply':
            case 'reply-all':
                return originalSubject.startsWith('Re:') ? originalSubject : `Re: ${originalSubject}`;
            case 'forward':
                return originalSubject.startsWith('Fwd:') ? originalSubject : `Fwd: ${originalSubject}`;
            default:
                return originalSubject;
        }
    }

    private convertToHtml(text: string): string {
        // Convert plain text to HTML, preserving line breaks and basic formatting
        let html = DOMPurify.sanitize(text);
        html = html.replace(/\n/g, '<br>');
        html = html.replace(/\*\*(.*?)\*\*/g, '<strong>$1</strong>'); // Bold
        html = html.replace(/\*(.*?)\*/g, '<em>$1</em>'); // Italic
        html = html.replace(/https?:\/\/[^\s]+/g, '<a href="$&" target="_blank">$&</a>'); // Links
        return html;
    }

    private selectAttachments() {
        const input = document.createElement('input');
        input.type = 'file';
        input.multiple = true;
        input.accept = '*/*';
        
        input.addEventListener('change', (e) => {
            const files = (e.target as HTMLInputElement).files;
            if (files) {
                this.addAttachments(Array.from(files));
            }
        });
        
        input.click();
    }

    private addAttachments(files: File[]) {
        // Check file size limit (25MB per file)
        const maxSize = 25 * 1024 * 1024;
        const validFiles = files.filter(file => {
            if (file.size > maxSize) {
                this.showNotification(`${file.name} exceeds 25MB limit`, 'warning');
                return false;
            }
            return true;
        });
        
        this.attachments.push(...validFiles);
        this.updateAttachmentsList();
    }

    private updateAttachmentsList() {
        const container = this.container.querySelector('.attachments-list');
        if (!container) {
            // Create attachments list if it doesn't exist
            const attachmentsList = document.createElement('div');
            attachmentsList.className = 'attachments-list';
            this.textarea.parentElement?.appendChild(attachmentsList);
        }
        
        const list = this.container.querySelector('.attachments-list');
        if (list) {
            list.innerHTML = this.attachments.map((file, index) => `
                <div class="attachment-item" data-index="${index}">
                    <i class="fas fa-file"></i>
                    <span class="attachment-name">${DOMPurify.sanitize(file.name)}</span>
                    <span class="attachment-size">${this.formatFileSize(file.size)}</span>
                    <button class="remove-attachment" data-index="${index}">
                        <i class="fas fa-times"></i>
                    </button>
                </div>
            `).join('');
            
            // Add remove handlers
            list.querySelectorAll('.remove-attachment').forEach(btn => {
                btn.addEventListener('click', (e) => {
                    const index = parseInt((e.currentTarget as HTMLElement).dataset.index || '0');
                    this.removeAttachment(index);
                });
            });
        }
    }

    private removeAttachment(index: number) {
        this.attachments.splice(index, 1);
        this.updateAttachmentsList();
    }

    private async uploadAttachments(): Promise<string[]> {
        if (this.attachments.length === 0) {
            return [];
        }
        
        const formData = new FormData();
        this.attachments.forEach(file => {
            formData.append('files', file);
        });
        
        try {
            const response = await this.api.uploadAttachments(formData);
            return response.attachment_ids;
        } catch (error) {
            console.error('Failed to upload attachments:', error);
            throw error;
        }
    }

    private autoResize() {
        this.textarea.style.height = 'auto';
        this.textarea.style.height = Math.min(this.textarea.scrollHeight, 300) + 'px';
    }

    private formatFileSize(bytes: number): string {
        if (bytes < 1024) return bytes + ' B';
        if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' KB';
        return (bytes / (1024 * 1024)).toFixed(1) + ' MB';
    }

    private showNotification(message: string, type: 'success' | 'error' | 'warning' | 'info') {
        // Create notification element
        const notification = document.createElement('div');
        notification.className = `notification notification-${type}`;
        notification.innerHTML = `
            <i class="fas fa-${
                type === 'success' ? 'check-circle' :
                type === 'error' ? 'exclamation-circle' :
                type === 'warning' ? 'exclamation-triangle' :
                'info-circle'
            }"></i>
            <span>${DOMPurify.sanitize(message)}</span>
        `;
        
        // Add to container
        this.container.appendChild(notification);
        
        // Animate in
        setTimeout(() => notification.classList.add('show'), 10);
        
        // Remove after 3 seconds
        setTimeout(() => {
            notification.classList.remove('show');
            setTimeout(() => notification.remove(), 300);
        }, 3000);
    }

    public focus() {
        this.textarea.focus();
    }

    public setContent(content: string) {
        this.textarea.value = content;
        this.autoResize();
    }

    public getContent(): string {
        return this.textarea.value;
    }
}