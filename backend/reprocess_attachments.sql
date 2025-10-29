-- Script to reprocess attachments from emails
-- This will scan all emails and extract attachment information

-- First, let's see current attachment count
SELECT COUNT(*) as total_attachments FROM attachments;

-- Check emails with attachments in their body
SELECT 
    id,
    subject,
    LENGTH(body_html) as html_length,
    LENGTH(body_text) as text_length,
    (body_html LIKE '%attachment%' OR body_text LIKE '%attachment%') as has_attachment_mention
FROM emails 
WHERE body_html LIKE '%attachment%' OR body_text LIKE '%attachment%'
LIMIT 10;

-- Note: Attachments need to be extracted from IMAP during email sync
-- This SQL script can't extract binary attachment data
-- You need to trigger a re-sync of emails to extract attachments

-- To force re-sync, you can:
-- 1. Delete all emails and let them re-sync
-- 2. Or update the last_sync timestamp to force a full re-sync

-- Option 2: Reset last sync (safer, won't delete emails)
UPDATE folders SET last_sync = NULL WHERE user_id = 1;

-- Check folder sync status
SELECT id, name, last_sync FROM folders WHERE user_id = 1;
