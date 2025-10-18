#!/usr/bin/env python3
"""
Simple test script to verify the mock IMAP/SMTP server is working
"""

import socket
import time

def test_imap(host='localhost', port=1143):
    """Test IMAP connection"""
    print("Testing IMAP connection...")
    try:
        sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        sock.settimeout(5)
        sock.connect((host, port))
        
        # Read greeting
        greeting = sock.recv(1024).decode()
        print(f"✓ IMAP Greeting: {greeting.strip()}")
        
        # Test LOGIN
        sock.send(b"A001 LOGIN test@example.com password123\r\n")
        response = sock.recv(1024).decode()
        print(f"✓ IMAP Login: {response.strip()}")
        
        # Test LIST
        sock.send(b"A002 LIST \"\" \"*\"\r\n")
        response = sock.recv(1024).decode()
        print(f"✓ IMAP List: {response.strip()}")
        
        # Test SELECT
        sock.send(b"A003 SELECT INBOX\r\n")
        response = sock.recv(1024).decode()
        print(f"✓ IMAP Select: {response.strip()}")
        
        # Test LOGOUT
        sock.send(b"A004 LOGOUT\r\n")
        response = sock.recv(1024).decode()
        print(f"✓ IMAP Logout: {response.strip()}")
        
        sock.close()
        print("✓ IMAP test completed successfully!\n")
        return True
        
    except Exception as e:
        print(f"✗ IMAP test failed: {e}\n")
        return False

def test_smtp(host='localhost', port=1587):
    """Test SMTP connection"""
    print("Testing SMTP connection...")
    try:
        sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        sock.settimeout(5)
        sock.connect((host, port))
        
        # Read greeting
        greeting = sock.recv(1024).decode()
        print(f"✓ SMTP Greeting: {greeting.strip()}")
        
        # Test HELO
        sock.send(b"HELO test.example.com\r\n")
        response = sock.recv(1024).decode()
        print(f"✓ SMTP Helo: {response.strip()}")
        
        # Test MAIL FROM
        sock.send(b"MAIL FROM: <test@example.com>\r\n")
        response = sock.recv(1024).decode()
        print(f"✓ SMTP Mail From: {response.strip()}")
        
        # Test RCPT TO
        sock.send(b"RCPT TO: <recipient@example.com>\r\n")
        response = sock.recv(1024).decode()
        print(f"✓ SMTP Rcpt To: {response.strip()}")
        
        # Test QUIT
        sock.send(b"QUIT\r\n")
        response = sock.recv(1024).decode()
        print(f"✓ SMTP Quit: {response.strip()}")
        
        sock.close()
        print("✓ SMTP test completed successfully!\n")
        return True
        
    except Exception as e:
        print(f"✗ SMTP test failed: {e}\n")
        return False

def main():
    print("=" * 60)
    print("Mock Email Server Connection Test")
    print("=" * 60)
    print()
    
    print("Make sure the mock server is running before running this test!")
    print("Start it with: python3 mock_server.py")
    print()
    
    input("Press Enter to start testing...")
    print()
    
    imap_ok = test_imap()
    smtp_ok = test_smtp()
    
    print("=" * 60)
    if imap_ok and smtp_ok:
        print("✓ All tests passed! The mock server is working correctly.")
    else:
        print("✗ Some tests failed. Check if the server is running.")
    print("=" * 60)

if __name__ == "__main__":
    main()
