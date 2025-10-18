#!/usr/bin/env python3
"""
Mock IMAP/SMTP Server for Testing Email Clients
A simple implementation that provides basic IMAP and SMTP functionality
"""

import socket
import threading
import time
import json
import os
from datetime import datetime

# Test data - pre-populated emails
TEST_EMAILS = [
    {
        "id": 1,
        "from": "alice@example.com",
        "to": "test@example.com",
        "subject": "Welcome to Frame Email Client",
        "date": "Mon, 15 Jan 2024 10:30:00 +0000",
        "body": "Hello! This is a test email to help you get started with Frame Email Client.",
        "thread_id": "thread-1"
    },
    {
        "id": 2,
        "from": "bob@example.com",
        "to": "test@example.com",
        "subject": "Re: Welcome to Frame Email Client",
        "date": "Mon, 15 Jan 2024 11:00:00 +0000",
        "body": "Thanks for the welcome! This looks great.",
        "thread_id": "thread-1"
    },
    {
        "id": 3,
        "from": "charlie@example.com",
        "to": "test@example.com",
        "subject": "Meeting Tomorrow",
        "date": "Mon, 15 Jan 2024 14:00:00 +0000",
        "body": "Don't forget about our meeting tomorrow at 2 PM.",
        "thread_id": "thread-2"
    },
    {
        "id": 4,
        "from": "alice@example.com",
        "to": "test@example.com",
        "subject": "Re: Meeting Tomorrow",
        "date": "Mon, 15 Jan 2024 15:30:00 +0000",
        "body": "I'll be there!",
        "thread_id": "thread-2"
    },
    {
        "id": 5,
        "from": "dave@example.com",
        "to": "test@example.com",
        "subject": "Project Update",
        "date": "Tue, 16 Jan 2024 09:00:00 +0000",
        "body": "Here's the latest update on the project. Everything is on track.",
        "thread_id": "thread-3"
    }
]

class MockIMAPServer:
    """Simple IMAP server implementation"""
    
    def __init__(self, host='localhost', port=1143):
        self.host = host
        self.port = port
        self.emails = TEST_EMAILS.copy()
        self.next_id = len(self.emails) + 1
        
    def start(self):
        """Start the IMAP server"""
        server = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        server.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        server.bind((self.host, self.port))
        server.listen(5)
        print(f"[IMAP] Mock IMAP server listening on {self.host}:{self.port}")
        
        while True:
            client, address = server.accept()
            print(f"[IMAP] Connection from {address}")
            client_thread = threading.Thread(target=self.handle_client, args=(client,))
            client_thread.daemon = True
            client_thread.start()
    
    def handle_client(self, client):
        """Handle IMAP client connection"""
        try:
            # Send greeting
            client.send(b"* OK Mock IMAP Server Ready\r\n")
            
            authenticated = False
            selected_folder = None
            
            while True:
                data = client.recv(4096).decode('utf-8', errors='ignore')
                if not data:
                    break
                
                print(f"[IMAP] Received: {data.strip()}")
                
                # Parse command
                parts = data.strip().split(' ', 2)
                if len(parts) < 2:
                    continue
                    
                tag = parts[0]
                command = parts[1].upper()
                args = parts[2] if len(parts) > 2 else ""
                
                # Handle commands
                if command == "CAPABILITY":
                    response = f"* CAPABILITY IMAP4rev1 AUTH=PLAIN\r\n{tag} OK CAPABILITY completed\r\n"
                    client.send(response.encode())
                    
                elif command == "LOGIN":
                    # Accept any credentials for testing
                    authenticated = True
                    response = f"{tag} OK LOGIN completed\r\n"
                    client.send(response.encode())
                    print(f"[IMAP] User authenticated")
                    
                elif command == "LIST":
                    # Return basic folder structure
                    response = '* LIST (\\HasNoChildren) "/" "INBOX"\r\n'
                    response += '* LIST (\\HasNoChildren) "/" "Sent"\r\n'
                    response += '* LIST (\\HasNoChildren) "/" "Drafts"\r\n'
                    response += f'{tag} OK LIST completed\r\n'
                    client.send(response.encode())
                    
                elif command == "SELECT":
                    selected_folder = "INBOX"
                    num_messages = len(self.emails)
                    response = f"* {num_messages} EXISTS\r\n"
                    response += f"* {num_messages} RECENT\r\n"
                    response += "* OK [UIDVALIDITY 1] UIDs valid\r\n"
                    response += f"* OK [UIDNEXT {self.next_id}] Predicted next UID\r\n"
                    response += f"{tag} OK [READ-WRITE] SELECT completed\r\n"
                    client.send(response.encode())
                    
                elif command == "FETCH":
                    # Simple FETCH implementation
                    if authenticated and selected_folder:
                        for email in self.emails:
                            msg_id = email['id']
                            response = f"* {msg_id} FETCH ("
                            
                            if "BODY" in args.upper() or "RFC822" in args.upper():
                                body = self.format_email(email)
                                response += f'RFC822 {{{len(body)}}}\r\n{body}'
                            
                            if "FLAGS" in args.upper():
                                response += 'FLAGS (\\Seen) '
                            
                            response += ")\r\n"
                            client.send(response.encode())
                        
                        response = f"{tag} OK FETCH completed\r\n"
                        client.send(response.encode())
                    
                elif command == "SEARCH":
                    # Return all message IDs
                    msg_ids = " ".join(str(e['id']) for e in self.emails)
                    response = f"* SEARCH {msg_ids}\r\n"
                    response += f"{tag} OK SEARCH completed\r\n"
                    client.send(response.encode())
                    
                elif command == "LOGOUT":
                    response = f"* BYE Mock IMAP Server logging out\r\n{tag} OK LOGOUT completed\r\n"
                    client.send(response.encode())
                    break
                    
                else:
                    response = f"{tag} BAD Command not recognized\r\n"
                    client.send(response.encode())
                    
        except Exception as e:
            print(f"[IMAP] Error: {e}")
        finally:
            client.close()
    
    def format_email(self, email):
        """Format email as RFC822 message"""
        msg = f"From: {email['from']}\r\n"
        msg += f"To: {email['to']}\r\n"
        msg += f"Subject: {email['subject']}\r\n"
        msg += f"Date: {email['date']}\r\n"
        msg += f"Message-ID: <{email['id']}@mock.example.com>\r\n"
        if 'thread_id' in email:
            msg += f"In-Reply-To: <{email['thread_id']}@mock.example.com>\r\n"
        msg += "\r\n"
        msg += email['body']
        msg += "\r\n"
        return msg


class MockSMTPServer:
    """Simple SMTP server implementation"""
    
    def __init__(self, host='localhost', port=1587):
        self.host = host
        self.port = port
        self.received_emails = []
        
    def start(self):
        """Start the SMTP server"""
        server = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        server.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        server.bind((self.host, self.port))
        server.listen(5)
        print(f"[SMTP] Mock SMTP server listening on {self.host}:{self.port}")
        
        while True:
            client, address = server.accept()
            print(f"[SMTP] Connection from {address}")
            client_thread = threading.Thread(target=self.handle_client, args=(client,))
            client_thread.daemon = True
            client_thread.start()
    
    def handle_client(self, client):
        """Handle SMTP client connection"""
        try:
            # Send greeting
            client.send(b"220 Mock SMTP Server Ready\r\n")
            
            mail_from = None
            rcpt_to = []
            data_mode = False
            email_data = []
            
            while True:
                data = client.recv(4096).decode('utf-8', errors='ignore')
                if not data:
                    break
                
                if data_mode:
                    if data.strip() == ".":
                        # End of email data
                        email_content = "".join(email_data)
                        self.received_emails.append({
                            'from': mail_from,
                            'to': rcpt_to,
                            'data': email_content,
                            'timestamp': datetime.now().isoformat()
                        })
                        print(f"[SMTP] Email received from {mail_from} to {rcpt_to}")
                        client.send(b"250 OK: Message accepted\r\n")
                        data_mode = False
                        email_data = []
                        mail_from = None
                        rcpt_to = []
                    else:
                        email_data.append(data)
                    continue
                
                print(f"[SMTP] Received: {data.strip()}")
                
                command = data.strip().upper()
                
                if command.startswith("HELO") or command.startswith("EHLO"):
                    response = "250 Mock SMTP Server\r\n"
                    client.send(response.encode())
                    
                elif command.startswith("MAIL FROM"):
                    mail_from = command.split(":", 1)[1].strip()
                    client.send(b"250 OK\r\n")
                    
                elif command.startswith("RCPT TO"):
                    rcpt = command.split(":", 1)[1].strip()
                    rcpt_to.append(rcpt)
                    client.send(b"250 OK\r\n")
                    
                elif command == "DATA":
                    client.send(b"354 Start mail input; end with <CRLF>.<CRLF>\r\n")
                    data_mode = True
                    
                elif command == "QUIT":
                    client.send(b"221 Bye\r\n")
                    break
                    
                else:
                    client.send(b"500 Command not recognized\r\n")
                    
        except Exception as e:
            print(f"[SMTP] Error: {e}")
        finally:
            client.close()


def main():
    """Start both IMAP and SMTP servers"""
    print("=" * 60)
    print("Mock Email Server for Frame Email Client Testing")
    print("=" * 60)
    print()
    
    # Start IMAP server in a thread
    imap_server = MockIMAPServer()
    imap_thread = threading.Thread(target=imap_server.start)
    imap_thread.daemon = True
    imap_thread.start()
    
    # Start SMTP server in a thread
    smtp_server = MockSMTPServer()
    smtp_thread = threading.Thread(target=smtp_server.start)
    smtp_thread.daemon = True
    smtp_thread.start()
    
    print()
    print("=" * 60)
    print("Test Credentials:")
    print("  Email: test@example.com")
    print("  Password: password123 (any password works)")
    print("  IMAP Host: localhost")
    print("  IMAP Port: 1143")
    print("  SMTP Host: localhost")
    print("  SMTP Port: 1587")
    print("=" * 60)
    print()
    print("Press Ctrl+C to stop the servers")
    print()
    
    try:
        while True:
            time.sleep(1)
    except KeyboardInterrupt:
        print("\n\nShutting down servers...")


if __name__ == "__main__":
    main()
