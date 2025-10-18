export class RichTextEditor {
    private container: HTMLElement;
    private toolbar: HTMLElement;
    private editor: HTMLDivElement;
    private sourceTextarea: HTMLTextAreaElement;
    private isSourceMode: boolean = false;
    
    constructor(textarea: HTMLTextAreaElement) {
        this.sourceTextarea = textarea;
        this.container = document.createElement('div');
        this.container.className = 'rich-text-editor';
        this.toolbar = this.createToolbar();
        this.editor = this.createEditor();
        
        this.setupEventListeners();
    }
    
    initialize() {
        // Hide original textarea
        this.sourceTextarea.style.display = 'none';
        
        // Insert editor after textarea
        this.sourceTextarea.parentNode?.insertBefore(this.container, this.sourceTextarea.nextSibling);
        
        // Add toolbar and editor to container
        this.container.appendChild(this.toolbar);
        this.container.appendChild(this.editor);
        
        // Set initial content
        this.editor.innerHTML = this.sourceTextarea.value || '';
    }
    
    private createToolbar(): HTMLElement {
        const toolbar = document.createElement('div');
        toolbar.className = 'editor-toolbar';
        
        const buttons = [
            { command: 'bold', icon: '𝐁', title: 'Bold (Ctrl+B)' },
            { command: 'italic', icon: '𝐼', title: 'Italic (Ctrl+I)' },
            { command: 'underline', icon: 'U̲', title: 'Underline (Ctrl+U)' },
            { command: 'strikethrough', icon: 'S̶', title: 'Strikethrough' },
            { separator: true },
            { command: 'insertUnorderedList', icon: '•', title: 'Bullet List' },
            { command: 'insertOrderedList', icon: '1.', title: 'Numbered List' },
            { command: 'outdent', icon: '⇤', title: 'Decrease Indent' },
            { command: 'indent', icon: '⇥', title: 'Increase Indent' },
            { separator: true },
            { command: 'createLink', icon: '🔗', title: 'Insert Link (Ctrl+K)' },
            { command: 'unlink', icon: '⛓️‍💥', title: 'Remove Link' },
            { command: 'insertImage', icon: '🖼️', title: 'Insert Image' },
            { separator: true },
            { command: 'removeFormat', icon: 'Ⓣ', title: 'Clear Formatting' },
            { command: 'viewSource', icon: '</>', title: 'View Source' },
        ];
        
        buttons.forEach(btn => {
            if (btn.separator) {
                const sep = document.createElement('span');
                sep.className = 'toolbar-separator';
                toolbar.appendChild(sep);
            } else {
                const button = document.createElement('button');
                button.className = 'toolbar-button';
                button.innerHTML = btn.icon;
                button.title = btn.title;
                button.dataset.command = btn.command;
                button.addEventListener('click', (e) => {
                    e.preventDefault();
                    this.executeCommand(btn.command);
                });
                toolbar.appendChild(button);
            }
        });
        
        return toolbar;
    }
    
    private createEditor(): HTMLDivElement {
        const editor = document.createElement('div');
        editor.className = 'editor-content';
        editor.contentEditable = 'true';
        editor.style.minHeight = '200px';
        editor.style.padding = '10px';
        editor.style.border = '1px solid #ddd';
        editor.style.borderRadius = '4px';
        editor.style.backgroundColor = '#fff';
        
        return editor;
    }
    
    private setupEventListeners() {
        // Sync content to textarea
        this.editor.addEventListener('input', () => {
            this.sourceTextarea.value = this.isSourceMode ? this.editor.textContent || '' : this.editor.innerHTML;
            this.sourceTextarea.dispatchEvent(new Event('input', { bubbles: true }));
        });
        
        // Handle keyboard shortcuts
        this.editor.addEventListener('keydown', (e) => {
            if (e.ctrlKey || e.metaKey) {
                switch (e.key) {
                    case 'b':
                        e.preventDefault();
                        this.executeCommand('bold');
                        break;
                    case 'i':
                        e.preventDefault();
                        this.executeCommand('italic');
                        break;
                    case 'u':
                        e.preventDefault();
                        this.executeCommand('underline');
                        break;
                    case 'k':
                        e.preventDefault();
                        this.executeCommand('createLink');
                        break;
                }
            }
        });
        
        // Handle paste events
        this.editor.addEventListener('paste', (e) => {
            e.preventDefault();
            const text = e.clipboardData?.getData('text/plain');
            if (text) {
                document.execCommand('insertText', false, text);
            }
        });
    }
    
    private executeCommand(command: string) {
        if (command === 'createLink') {
            const url = prompt('Enter URL:');
            if (url) {
                document.execCommand('createLink', false, url);
            }
        } else if (command === 'insertImage') {
            const url = prompt('Enter image URL:');
            if (url) {
                document.execCommand('insertImage', false, url);
            }
        } else if (command === 'viewSource') {
            this.toggleSourceMode();
        } else {
            document.execCommand(command, false);
        }
        
        this.editor.focus();
    }
    
    private toggleSourceMode() {
        this.isSourceMode = !this.isSourceMode;
        
        if (this.isSourceMode) {
            const html = this.editor.innerHTML;
            this.editor.textContent = html;
            this.editor.style.fontFamily = 'monospace';
            this.editor.style.whiteSpace = 'pre-wrap';
        } else {
            const text = this.editor.textContent || '';
            this.editor.innerHTML = text;
            this.editor.style.fontFamily = '';
            this.editor.style.whiteSpace = '';
        }
    }
    
    public getHTML(): string {
        return this.isSourceMode ? (this.editor.textContent || '') : this.editor.innerHTML;
    }
    
    public getText(): string {
        return this.editor.textContent || '';
    }
    
    public setHTML(html: string) {
        this.editor.innerHTML = html;
        this.sourceTextarea.value = html;
    }
    
    public clear() {
        this.editor.innerHTML = '';
        this.sourceTextarea.value = '';
    }
    
    public focus() {
        this.editor.focus();
    }
}