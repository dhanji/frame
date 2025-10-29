#!/bin/bash

# Force Attachment Re-sync Script
# This script will trigger a full re-sync of emails to extract attachments

set -e

DB_PATH="email_client.db"

echo "🔄 Force Attachment Re-sync"
echo "============================="
echo ""

# Check current attachment count
echo "📊 Current Statistics:"
echo "Attachments: $(sqlite3 $DB_PATH 'SELECT COUNT(*) FROM attachments;')"
echo "Emails: $(sqlite3 $DB_PATH 'SELECT COUNT(*) FROM emails;')"
echo ""

# Ask for confirmation
read -p "This will reset folder sync timestamps to force re-sync. Continue? (y/N) " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo "❌ Cancelled"
    exit 1
fi

echo ""
echo "🔧 Resetting folder sync timestamps..."
sqlite3 $DB_PATH "UPDATE folders SET last_sync = NULL WHERE user_id = 1;"

echo "✅ Sync timestamps reset"
echo ""
echo "📧 Folder Status:"
sqlite3 $DB_PATH "SELECT name, last_sync FROM folders WHERE user_id = 1;"

echo ""
echo "✅ Done! The email sync service will now re-process all emails."
echo "   Attachments will be extracted during the next sync cycle."
echo "   Check the backend logs to monitor progress."
echo ""
echo "💡 Tip: Restart the backend server to trigger immediate sync:"
echo "   cd .. && ./run.sh restart"
