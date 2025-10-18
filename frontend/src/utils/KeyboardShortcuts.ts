export class KeyboardShortcuts {
    private shortcuts: Map<string, () => void> = new Map();
    private enabled: boolean = false;
    private currentSequence: string[] = [];
    private sequenceTimer: NodeJS.Timeout | null = null;
    
    constructor() {
        this.handleKeyDown = this.handleKeyDown.bind(this);
    }
    
    register(shortcut: string, callback: () => void) {
        this.shortcuts.set(shortcut.toLowerCase(), callback);
    }
    
    unregister(shortcut: string) {
        this.shortcuts.delete(shortcut.toLowerCase());
    }
    
    enable() {
        if (!this.enabled) {
            document.addEventListener('keydown', this.handleKeyDown);
            this.enabled = true;
        }
    }
    
    disable() {
        if (this.enabled) {
            document.removeEventListener('keydown', this.handleKeyDown);
            this.enabled = false;
        }
    }
    
    private handleKeyDown(e: KeyboardEvent) {
        // Don't trigger shortcuts when typing in input fields
        const target = e.target as HTMLElement;
        if (target.tagName === 'INPUT' || 
            target.tagName === 'TEXTAREA' || 
            target.contentEditable === 'true') {
            // Allow some global shortcuts even in input fields
            if (!e.ctrlKey && !e.metaKey) {
                return;
            }
        }
        
        const key = this.getKeyString(e);
        
        // Handle sequence shortcuts (e.g., 'g i' for go to inbox)
        if (this.sequenceTimer) {
            clearTimeout(this.sequenceTimer);
            this.sequenceTimer = null;
        }
        
        this.currentSequence.push(key);
        
        // Check if current sequence matches any shortcut
        const sequenceString = this.currentSequence.join(' ');
        
        if (this.shortcuts.has(sequenceString)) {
            e.preventDefault();
            const callback = this.shortcuts.get(sequenceString);
            if (callback) {
                callback();
            }
            this.currentSequence = [];
        } else {
            // Check if this could be the start of a sequence
            let possibleSequence = false;
            for (const [shortcut] of this.shortcuts) {
                if (shortcut.startsWith(sequenceString)) {
                    possibleSequence = true;
                    break;
                }
            }
            
            if (possibleSequence) {
                // Wait for next key
                this.sequenceTimer = setTimeout(() => {
                    this.currentSequence = [];
                }, 1000);
            } else {
                // Check single key shortcut
                if (this.shortcuts.has(key)) {
                    e.preventDefault();
                    const callback = this.shortcuts.get(key);
                    if (callback) {
                        callback();
                    }
                }
                this.currentSequence = [];
            }
        }
    }
    
    private getKeyString(e: KeyboardEvent): string {
        const parts: string[] = [];
        
        if (e.ctrlKey) parts.push('ctrl');
        if (e.altKey) parts.push('alt');
        if (e.shiftKey) parts.push('shift');
        if (e.metaKey) parts.push('cmd');
        
        let key = e.key.toLowerCase();
        
        // Normalize key names
        const keyMap: Record<string, string> = {
            ' ': 'space',
            'arrowup': 'up',
            'arrowdown': 'down',
            'arrowleft': 'left',
            'arrowright': 'right',
            'escape': 'esc',
            'delete': 'delete',
            'backspace': 'backspace',
            'enter': 'enter',
            'tab': 'tab',
        };
        
        if (keyMap[key]) {
            key = keyMap[key];
        }
        
        // Don't add modifier keys as the main key
        if (!['control', 'alt', 'shift', 'meta'].includes(key)) {
            parts.push(key);
        }
        
        return parts.join('+');
    }
    
    public showHelp() {
        const helpModal = document.createElement('div');
        helpModal.className = 'keyboard-shortcuts-help modal active';
        
        const shortcuts = [
            { keys: 'Ctrl+N', description: 'Compose new email' },
            { keys: 'Ctrl+R', description: 'Refresh emails' },
            { keys: 'Ctrl+F', description: 'Search' },
            { keys: 'J', description: 'Next conversation' },
            { keys: 'K', description: 'Previous conversation' },
            { keys: 'O', description: 'Open/close conversation' },
            { keys: 'R', description: 'Reply' },
            { keys: 'A', description: 'Reply all' },
            { keys: 'F', description: 'Forward' },
            { keys: 'Delete', description: 'Delete email' },
            { keys: 'U', description: 'Toggle read/unread' },
            { keys: 'S', description: 'Star/unstar' },
            { keys: 'G then I', description: 'Go to Inbox' },
            { keys: 'G then S', description: 'Go to Sent' },
            { keys: 'G then D', description: 'Go to Drafts' },
            { keys: 'G then T', description: 'Go to Trash' },
            { keys: '/', description: 'Focus search' },
            { keys: '?', description: 'Show this help' },
        ];
        
        helpModal.innerHTML = `
            <div class="modal-content" style="max-width: 500px;">
                <div class="modal-header">
                    <h2>Keyboard Shortcuts</h2>
                    <button class="close-btn" onclick="this.closest('.modal').remove()">&times;</button>
                </div>
                <div class="modal-body" style="padding: 20px;">
                    <table style="width: 100%;">
                        ${shortcuts.map(s => `
                            <tr>
                                <td style="padding: 5px 10px; font-family: monospace; font-weight: bold;">${s.keys}</td>
                                <td style="padding: 5px 10px;">${s.description}</td>
                            </tr>
                        `).join('')}
                    </table>
                </div>
            </div>
        `;
        
        document.body.appendChild(helpModal);
        
        // Close on escape
        const closeHandler = (e: KeyboardEvent) => {
            if (e.key === 'Escape') {
                helpModal.remove();
                document.removeEventListener('keydown', closeHandler);
            }
        };
        document.addEventListener('keydown', closeHandler);
        
        // Close on click outside
        helpModal.addEventListener('click', (e) => {
            if (e.target === helpModal) {
                helpModal.remove();
            }
        });
    }
}

// Export singleton instance
export const keyboardShortcuts = new KeyboardShortcuts();