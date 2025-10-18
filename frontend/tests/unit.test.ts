import { describe, it, expect, beforeEach, vi } from 'vitest';
import { apiClient } from '../src/api/client';
import { formatDate, truncateText, extractEmailName, debounce } from '../src/utils/helpers';
import { KeyboardShortcuts } from '../src/utils/KeyboardShortcuts';
import { RichTextEditor } from '../src/components/RichTextEditor';

// Mock fetch for API tests
global.fetch = vi.fn();

describe('API Client', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    localStorage.clear();
  });

  it('should login successfully', async () => {
    const mockResponse = {
      token: 'test-token',
      user: { id: '1', email: 'test@example.com', name: 'Test User' }
    };

    (global.fetch as any).mockResolvedValueOnce({
      ok: true,
      json: async () => mockResponse
    });

    const result = await apiClient.login('test@example.com', 'password');
    expect(result).toEqual(mockResponse);
    expect(apiClient.getToken()).toBe('test-token');
  });

  it('should handle login failure', async () => {
    (global.fetch as any).mockResolvedValueOnce({
      ok: false,
      status: 401,
      json: async () => ({ error: 'Invalid credentials' })
    });

    await expect(apiClient.login('wrong@example.com', 'wrong')).rejects.toThrow();
  });

  it('should fetch conversations', async () => {
    const mockConversations = [
      { id: '1', subject: 'Test', messages: [] }
    ];

    (global.fetch as any).mockResolvedValueOnce({
      ok: true,
      json: async () => mockConversations
    });

    apiClient.setToken('test-token');
    const result = await apiClient.getConversations();
    expect(result).toEqual(mockConversations);
  });

  it('should send email', async () => {
    const mockResponse = { message_id: '123' };
    const emailData = {
      to: ['recipient@example.com'],
      subject: 'Test',
      body: 'Test email'
    };

    (global.fetch as any).mockResolvedValueOnce({
      ok: true,
      json: async () => mockResponse
    });

    apiClient.setToken('test-token');
    const result = await apiClient.sendEmail(emailData);
    expect(result).toEqual(mockResponse);
  });
});

describe('Helper Functions', () => {
  it('should format date correctly', () => {
    const date = new Date('2024-01-01T12:00:00Z');
    const formatted = formatDate(date);
    expect(formatted).toMatch(/Jan 1/);
  });

  it('should truncate text', () => {
    const text = 'This is a very long text that needs to be truncated';
    const truncated = truncateText(text, 20);
    expect(truncated).toBe('This is a very long...');
    expect(truncated.length).toBeLessThanOrEqual(23);
  });

  it('should extract email name', () => {
    expect(extractEmailName('John Doe <john@example.com>')).toBe('John Doe');
    expect(extractEmailName('jane@example.com')).toBe('jane');
    expect(extractEmailName('"Smith, Bob" <bob@example.com>')).toBe('Smith, Bob');
  });

  it('should debounce function calls', async () => {
    const mockFn = vi.fn();
    const debouncedFn = debounce(mockFn, 100);

    debouncedFn();
    debouncedFn();
    debouncedFn();

    expect(mockFn).not.toHaveBeenCalled();

    await new Promise(resolve => setTimeout(resolve, 150));
    expect(mockFn).toHaveBeenCalledTimes(1);
  });
});

describe('Keyboard Shortcuts', () => {
  let shortcuts: KeyboardShortcuts;

  beforeEach(() => {
    shortcuts = new KeyboardShortcuts();
  });

  it('should register and trigger shortcuts', () => {
    const mockCallback = vi.fn();
    shortcuts.register('ctrl+n', mockCallback);
    shortcuts.enable();

    const event = new KeyboardEvent('keydown', {
      key: 'n',
      ctrlKey: true
    });

    document.dispatchEvent(event);
    expect(mockCallback).toHaveBeenCalled();
  });

  it('should not trigger when disabled', () => {
    const mockCallback = vi.fn();
    shortcuts.register('ctrl+n', mockCallback);
    shortcuts.disable();

    const event = new KeyboardEvent('keydown', {
      key: 'n',
      ctrlKey: true
    });

    document.dispatchEvent(event);
    expect(mockCallback).not.toHaveBeenCalled();
  });

  it('should handle key sequences', () => {
    const mockCallback = vi.fn();
    shortcuts.register('g i', mockCallback);
    shortcuts.enable();

    const event1 = new KeyboardEvent('keydown', { key: 'g' });
    const event2 = new KeyboardEvent('keydown', { key: 'i' });

    document.dispatchEvent(event1);
    document.dispatchEvent(event2);
    expect(mockCallback).toHaveBeenCalled();
  });
});

describe('Rich Text Editor', () => {
  let textarea: HTMLTextAreaElement;
  let editor: RichTextEditor;

  beforeEach(() => {
    document.body.innerHTML = '<textarea id="test-editor"></textarea>';
    textarea = document.getElementById('test-editor') as HTMLTextAreaElement;
    editor = new RichTextEditor(textarea);
  });

  it('should initialize editor', () => {
    editor.initialize();
    const editorDiv = document.querySelector('.rich-text-editor');
    expect(editorDiv).toBeTruthy();
    expect(editorDiv?.getAttribute('contenteditable')).toBe('true');
  });

  it('should format text', () => {
    editor.initialize();
    const editorDiv = document.querySelector('.rich-text-editor') as HTMLDivElement;
    editorDiv.innerHTML = 'Test text';
    
    // Select all text
    const range = document.createRange();
    range.selectNodeContents(editorDiv);
    const selection = window.getSelection();
    selection?.removeAllRanges();
    selection?.addRange(range);

    editor.formatText('bold');
    expect(editorDiv.innerHTML).toContain('<b>');
  });

  it('should get HTML and plain text', () => {
    editor.initialize();
    const editorDiv = document.querySelector('.rich-text-editor') as HTMLDivElement;
    editorDiv.innerHTML = '<b>Bold</b> and <i>italic</i> text';

    expect(editor.getHTML()).toBe('<b>Bold</b> and <i>italic</i> text');
    expect(editor.getPlainText()).toBe('Bold and italic text');
  });
});

describe('Email Validation', () => {
  it('should validate email addresses', () => {
    const validEmails = [
      'user@example.com',
      'user.name@example.com',
      'user+tag@example.co.uk',
      'user_name@example-domain.com'
    ];

    const invalidEmails = [
      'invalid',
      '@example.com',
      'user@',
      'user @example.com',
      'user@example .com'
    ];

    const emailRegex = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;

    validEmails.forEach(email => {
      expect(emailRegex.test(email)).toBe(true);
    });

    invalidEmails.forEach(email => {
      expect(emailRegex.test(email)).toBe(false);
    });
  });
});

describe('Conversation Threading', () => {
  it('should group messages by thread', () => {
    const messages = [
      { id: '1', subject: 'Test', thread_id: 'thread1' },
      { id: '2', subject: 'Re: Test', thread_id: 'thread1' },
      { id: '3', subject: 'Another', thread_id: 'thread2' },
      { id: '4', subject: 'Re: Test', thread_id: 'thread1' }
    ];

    const threads = messages.reduce((acc, msg) => {
      if (!acc[msg.thread_id]) {
        acc[msg.thread_id] = [];
      }
      acc[msg.thread_id].push(msg);
      return acc;
    }, {} as Record<string, any[]>);

    expect(Object.keys(threads).length).toBe(2);
    expect(threads['thread1'].length).toBe(3);
    expect(threads['thread2'].length).toBe(1);
  });
});

describe('Draft Auto-save', () => {
  it('should auto-save draft after delay', async () => {
    const saveDraft = vi.fn();
    const debouncedSave = debounce(saveDraft, 1000);

    // Simulate typing
    debouncedSave();
    await new Promise(resolve => setTimeout(resolve, 500));
    debouncedSave(); // Reset timer
    await new Promise(resolve => setTimeout(resolve, 500));
    debouncedSave(); // Reset timer again

    expect(saveDraft).not.toHaveBeenCalled();

    await new Promise(resolve => setTimeout(resolve, 1100));
    expect(saveDraft).toHaveBeenCalledTimes(1);
  });
});