import { format, formatDistanceToNow, parseISO } from 'date-fns';

export function formatDate(dateString: string): string {
    const date = parseISO(dateString);
    const now = new Date();
    const diffInHours = (now.getTime() - date.getTime()) / (1000 * 60 * 60);
    
    if (diffInHours < 24) {
        return formatDistanceToNow(date, { addSuffix: true });
    } else if (diffInHours < 168) { // 7 days
        return format(date, 'EEEE');
    } else {
        return format(date, 'MMM d, yyyy');
    }
}

export function formatFullDate(dateString: string): string {
    return format(parseISO(dateString), 'PPpp');
}

export function truncateText(text: string, maxLength: number): string {
    if (text.length <= maxLength) return text;
    return text.substring(0, maxLength) + '...';
}

export function extractEmailName(email: string): string {
    const match = email.match(/^([^@]+)/);
    return match ? match[1] : email;
}

export function parseEmailAddresses(addresses: string): string[] {
    try {
        return JSON.parse(addresses);
    } catch {
        return addresses.split(',').map(a => a.trim()).filter(Boolean);
    }
}

export function debounce<T extends (...args: any[]) => any>(
    func: T,
    wait: number
): (...args: Parameters<T>) => void {
    let timeout: NodeJS.Timeout;
    
    return function executedFunction(...args: Parameters<T>) {
        const later = () => {
            clearTimeout(timeout);
            func(...args);
        };
        
        clearTimeout(timeout);
        timeout = setTimeout(later, wait);
    };
}

export function sanitizeHtml(html: string): string {
    // In production, use DOMPurify
    const div = document.createElement('div');
    div.textContent = html;
    return div.innerHTML;
}

export function showNotification(message: string, type: 'success' | 'error' | 'info' = 'info') {
    // Simple notification - in production, use a proper notification library
    const notification = document.createElement('div');
    notification.className = `notification notification-${type}`;
    notification.textContent = message;
    
    notification.style.cssText = `
        position: fixed;
        top: 20px;
        right: 20px;
        padding: 12px 20px;
        background: ${type === 'error' ? '#dc3545' : type === 'success' ? '#28a745' : '#007bff'};
        color: white;
        border-radius: 4px;
        box-shadow: 0 2px 10px rgba(0,0,0,0.2);
        z-index: 10000;
        animation: slideIn 0.3s ease;
    `;
    
    document.body.appendChild(notification);
    
    setTimeout(() => {
        notification.style.animation = 'slideOut 0.3s ease';
        setTimeout(() => notification.remove(), 300);
    }, 3000);
}

export function createLoadingSpinner(): HTMLElement {
    const spinner = document.createElement('div');
    spinner.className = 'loading';
    spinner.innerHTML = '<div class="spinner"></div><p>Loading...</p>';
    return spinner;
}

export function handleApiError(error: any): string {
    if (error.response?.data?.error) {
        return error.response.data.error;
    }
    if (error.message) {
        return error.message;
    }
    return 'An unexpected error occurred';
}