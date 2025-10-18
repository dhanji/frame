import axios, { AxiosInstance } from 'axios';
// import { io, Socket } from 'socket.io-client';

interface ApiResponse<T> {
    data: T;
    message?: string;
    error?: string;
}

export class EmailAPI {
    private api: AxiosInstance;
    // private socket: Socket | null = null;
    private csrfToken: string = '';
    private connectionPool: Map<string, Promise<any>> = new Map();
    private maxPoolSize: number = 10;
    private requestQueue: Array<() => Promise<any>> = [];
    private activeRequests: number = 0;

    constructor(baseURL: string = '/api') {
        this.api = axios.create({
            baseURL,
            timeout: 30000,
            headers: {
                'Content-Type': 'application/json',
            },
            withCredentials: true,
        });

        this.setupInterceptors();
        // this.initializeWebSocket();
        this.fetchCsrfToken();
    }

    private setupInterceptors() {
        // Request interceptor to add auth token and CSRF token
        this.api.interceptors.request.use(
            (config) => {
                const token = localStorage.getItem('auth_token');
                if (token) {
                    config.headers.Authorization = `Bearer ${token}`;
                }
                
                // Add CSRF token for non-GET requests
                if (config.method !== 'get' && this.csrfToken) {
                    config.headers['X-CSRF-Token'] = this.csrfToken;
                }
                
                return config;
            },
            (error) => {
                return Promise.reject(error);
            }
        );

        // Response interceptor for error handling
        this.api.interceptors.response.use(
            (response) => response,
            async (error) => {
                if (error.response?.status === 401) {
                    // Token expired, redirect to login
                    localStorage.removeItem('auth_token');
                    window.location.href = '/login';
                } else if (error.response?.status === 403 && error.response?.data?.error?.includes('CSRF')) {
                    // CSRF token invalid, refresh and retry
                    await this.fetchCsrfToken();
                    error.config.headers['X-CSRF-Token'] = this.csrfToken;
                    return this.api.request(error.config);
                }
                return Promise.reject(error);
            }
        );
    }

    private async fetchCsrfToken() {
        try {
            const response = await this.api.get('/csrf-token');
            this.csrfToken = response.data.token;
        } catch (error) {
            console.error('Failed to fetch CSRF token:', error);
        }
    }

    public async getCsrfToken(): Promise<string> {
        if (!this.csrfToken) {
            await this.fetchCsrfToken();
        }
        return this.csrfToken;
    }

    // private initializeWebSocket() {
    //     const wsUrl = import.meta.env.VITE_WS_URL || 'ws://localhost:8080/ws';
    //     
    //     this.socket = io(wsUrl, {
    //         transports: ['websocket'],
    //         reconnection: true,
    //         reconnectionDelay: 1000,
    //         reconnectionAttempts: 5,
    //     });

    //     this.socket.on('connect', () => {
    //         console.log('WebSocket connected');
    //         const token = localStorage.getItem('auth_token');
    //         if (token) {
    //             this.socket?.emit('authenticate', { token });
    //         }
    //     });

    //     this.socket.on('new_email', (data) => {
    //         // Dispatch custom event for new email
    //         window.dispatchEvent(new CustomEvent('new_email', { detail: data }));
    //     });

    //     this.socket.on('email_update', (data) => {
    //         // Dispatch custom event for email updates
    //         window.dispatchEvent(new CustomEvent('email_update', { detail: data }));
    //     });

    //     this.socket.on('disconnect', () => {
    //         console.log('WebSocket disconnected');
    //     });
    // }

    // Connection pooling for API requests
    private async executeWithPool<T>(key: string, request: () => Promise<T>): Promise<T> {
        // Check if request is already in pool
        if (this.connectionPool.has(key)) {
            return this.connectionPool.get(key) as Promise<T>;
        }

        // Check if we've reached max pool size
        if (this.activeRequests >= this.maxPoolSize) {
            // Queue the request
            return new Promise((resolve, reject) => {
                this.requestQueue.push(async () => {
                    try {
                        const result = await request();
                        resolve(result);
                    } catch (error) {
                        reject(error);
                    }
                });
            });
        }

        // Execute request
        this.activeRequests++;
        const promise = request().finally(() => {
            this.activeRequests--;
            this.connectionPool.delete(key);
            
            // Process queued requests
            if (this.requestQueue.length > 0) {
                const nextRequest = this.requestQueue.shift();
                nextRequest?.();
            }
        });

        this.connectionPool.set(key, promise);
        return promise;
    }

    // Authentication
    public async login(username: string, password: string): Promise<ApiResponse<any>> {
        const response = await this.api.post('/login', { username, password });
        if (response.data.token) {
            localStorage.setItem('auth_token', response.data.token);
            localStorage.setItem('user', JSON.stringify(response.data.user));
        }
        return response.data;
    }

    public async logout(): Promise<void> {
        // await this.api.post('/auth/logout');
        localStorage.removeItem('auth_token');
        localStorage.removeItem('user');
        // this.socket?.disconnect();
    }

    public async register(data: any): Promise<ApiResponse<any>> {
        const response = await this.api.post('/register', data);
        return response.data;
    }

    // Conversations
    public async getConversations(folder: string = 'INBOX', limit: number = 50, offset: number = 0): Promise<ApiResponse<any>> {
        return this.executeWithPool(
            `conversations-${folder}-${limit}-${offset}`,
            async () => {
                const response = await this.api.get('/conversations', {
                    params: { folder, limit, offset }
                });
                return response.data;
            }
        );
    }

    public async getConversation(id: string): Promise<any> {
        return this.executeWithPool(
            `conversation-${id}`,
            async () => {
                const response = await this.api.get(`/conversations/${id}`);
                return response.data;
            }
        );
    }

    public async bulkAction(data: any): Promise<ApiResponse<any>> {
        const response = await this.api.post('/conversations/bulk', data);
        return response.data;
    }

    // Emails
    public async sendEmail(data: any): Promise<ApiResponse<any>> {
        const response = await this.api.post('/emails/send', data);
        return response.data;
    }

    public async sendReply(data: any): Promise<ApiResponse<any>> {
        const response = await this.api.post(`/emails/${data.conversation_id}/reply`, data);
        return response.data;
    }

    public async markAsRead(emailId: string): Promise<ApiResponse<any>> {
        const response = await this.api.put(`/emails/${emailId}/read`, { is_read: true });
        return response.data;
    }

    public async deleteEmail(emailId: string): Promise<ApiResponse<any>> {
        const response = await this.api.delete(`/emails/${emailId}`);
        return response.data;
    }

    public async moveEmail(emailId: string, folder: string): Promise<ApiResponse<any>> {
        const response = await this.api.post(`/emails/${emailId}/move`, { folder });
        return response.data;
    }

    // Drafts
    public async getDrafts(): Promise<any[]> {
        return this.executeWithPool(
            'drafts',
            async () => {
                const response = await this.api.get('/drafts');
                return response.data;
            }
        );
    }

    public async saveDraft(data: any): Promise<any> {
        const response = await this.api.post('/drafts/auto-save', data);
        return response.data;
    }

    public async deleteDraft(id: string): Promise<ApiResponse<any>> {
        const response = await this.api.delete(`/drafts/${id}`);
        return response.data;
    }

    // Folders
    public async getFolders(): Promise<any[]> {
        return this.executeWithPool(
            'folders',
            async () => {
                const response = await this.api.get('/folders');
                return response.data;
            }
        );
    }

    public async createFolder(name: string): Promise<ApiResponse<any>> {
        const response = await this.api.post('/folders', { name });
        return response.data;
    }

    // Search
    public async searchEmails(query: any): Promise<any[]> {
        const response = await this.api.get('/search', { params: query });
        return response.data;
    }

    public async saveSearch(name: string, query: any): Promise<ApiResponse<any>> {
        const response = await this.api.post('/search/save', { name, query });
        return response.data;
    }

    public async getSavedSearches(): Promise<any[]> {
        return this.executeWithPool(
            'saved-searches',
            async () => {
                const response = await this.api.get('/search/saved');
                return response.data;
            }
        );
    }

    // Attachments
    public async uploadAttachments(formData: FormData): Promise<any> {
        const response = await this.api.post('/attachments/upload', formData, {
            headers: {
                'Content-Type': 'multipart/form-data',
            },
        });
        return response.data;
    }

    public async downloadAttachment(id: string): Promise<Blob> {
        const response = await this.api.get(`/attachments/${id}`, {
            responseType: 'blob',
        });
        return response.data;
    }

    // Settings
    public async getSettings(): Promise<any> {
        return this.executeWithPool(
            'settings',
            async () => {
                const response = await this.api.get('/settings');
                return response.data;
            }
        );
    }

    public async updateSettings(settings: any): Promise<ApiResponse<any>> {
        const response = await this.api.put('/settings', settings);
        return response.data;
    }

    // Filters
    public async getFilters(): Promise<any[]> {
        return this.executeWithPool(
            'filters',
            async () => {
                const response = await this.api.get('/filters');
                return response.data;
            }
        );
    }

    public async createFilter(filter: any): Promise<ApiResponse<any>> {
        const response = await this.api.post('/filters', filter);
        return response.data;
    }

    public async updateFilter(id: string, filter: any): Promise<ApiResponse<any>> {
        const response = await this.api.put(`/filters/${id}`, filter);
        return response.data;
    }

    public async deleteFilter(id: string): Promise<ApiResponse<any>> {
        const response = await this.api.delete(`/filters/${id}`);
        return response.data;
    }

    // Cleanup
    public disconnect() {
        // this.socket?.disconnect();
        this.connectionPool.clear();
        this.requestQueue = [];
    }
}