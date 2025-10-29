#!/bin/bash

# Test CalDAV sync by checking logs
echo "Checking if CalDAV sync service is running..."
echo ""

# Check if the service started
if grep -q "🗓️.*CalDAV" /tmp/backend_new.log 2>/dev/null; then
    echo "✅ CalDAV sync service logs found:"
    grep "🗓️" /tmp/backend_new.log | tail -10
else
    echo "❌ No CalDAV sync service logs found"
    echo ""
    echo "Checking main.rs for CalDAV spawn:"
    grep -A5 "CalDAV sync service" backend/src/main.rs
    echo ""
    echo "Checking if caldav_sync module exists:"
    ls -la backend/src/services/caldav_sync.rs
fi

echo ""
echo "Checking for any CalDAV-related logs:"
grep -i "calendar\|caldav" /tmp/backend_new.log 2>/dev/null | tail -20
