use chrono::{DateTime, Utc};
use icalendar::{Calendar, Component, Event, EventLike};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalendarEvent {
    pub uid: String,
    pub title: String,
    pub description: Option<String>,
    pub location: Option<String>,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub all_day: bool,
    pub attendees: Vec<String>,
    pub recurrence: Option<String>,
    pub etag: Option<String>,
}

pub struct CalDavClient {
    client: Client,
    base_url: String,
    username: String,
    password: String,
    calendar_path: String,
}

impl CalDavClient {
    pub fn new(
        base_url: String,
        username: String,
        password: String,
        calendar_path: Option<String>,
    ) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap();

        // If base_url already contains a path (like /events), don't add another path
        let calendar_path = if base_url.ends_with("/events") || base_url.ends_with("/events/") 
                              || base_url.ends_with("/user") || base_url.contains("/calendar/dav/") {
            String::new()  // Don't append anything
        } else {
            calendar_path.unwrap_or_else(|| "/calendar/".to_string())
        };

        log::info!("CalDavClient initialized: base_url={}, calendar_path='{}', final_url={}{}", base_url, calendar_path, base_url, calendar_path);

        Self {
            client,
            base_url,
            username,
            password,
            calendar_path,
        }
    }

    /// Test connection to CalDAV server
    pub async fn test_connection(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}{}", self.base_url, self.calendar_path);
        
        let response = self
            .client
            .request(http::Method::from_bytes(b"PROPFIND")?, &url)
            .basic_auth(&self.username, Some(&self.password))
            .header("Depth", "0")
            .header("Content-Type", "application/xml")
            .body(r#"<?xml version="1.0" encoding="utf-8" ?>
                <propfind xmlns="DAV:">
                    <prop>
                        <displayname />
                    </prop>
                </propfind>"#)
            .send()
            .await?;

        if response.status().is_success() || response.status() == StatusCode::MULTI_STATUS {
            Ok(())
        } else {
            Err(format!("CalDAV connection failed: {}", response.status()).into())
        }
    }

    /// Fetch all events from the calendar
    pub async fn fetch_events(
        &self,
    ) -> Result<Vec<CalendarEvent>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}{}", self.base_url, self.calendar_path);

        // CalDAV REPORT request to get all events
        let report_body = r#"<?xml version="1.0" encoding="utf-8" ?>
            <C:calendar-query xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
                <D:prop>
                    <D:getetag />
                    <C:calendar-data />
                </D:prop>
                <C:filter>
                    <C:comp-filter name="VCALENDAR">
                        <C:comp-filter name="VEVENT" />
                    </C:comp-filter>
                </C:filter>
            </C:calendar-query>"#;

        let response = self
            .client
            .request(http::Method::from_bytes(b"REPORT")?, &url)
            .basic_auth(&self.username, Some(&self.password))
            .header("Depth", "1")
            .header("Content-Type", "application/xml")
            .body(report_body)
            .send()
            .await?;

        if !response.status().is_success() && response.status() != StatusCode::MULTI_STATUS {
            return Err(format!("Failed to fetch events: {}", response.status()).into());
        }

        let body = response.text().await?;
        self.parse_calendar_response(&body)
    }

    /// Create a new event on the CalDAV server
    pub async fn create_event(
        &self,
        event: &CalendarEvent,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let ical_event = self.event_to_ical(event);
        let mut calendar = Calendar::new();
        calendar.push(ical_event);

        let ical_data = calendar.to_string();
        let event_url = format!("{}{}{}.ics", self.base_url, self.calendar_path, event.uid);

        let response = self
            .client
            .put(&event_url)
            .basic_auth(&self.username, Some(&self.password))
            .header("Content-Type", "text/calendar; charset=utf-8")
            .body(ical_data)
            .send()
            .await?;

        if response.status().is_success() || response.status() == StatusCode::CREATED {
            let etag = response
                .headers()
                .get("etag")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string());
            Ok(etag.unwrap_or_default())
        } else {
            Err(format!("Failed to create event: {}", response.status()).into())
        }
    }

    /// Update an existing event
    pub async fn update_event(
        &self,
        event: &CalendarEvent,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        // For updates, we use the same PUT method but with If-Match header if etag is available
        let ical_event = self.event_to_ical(event);
        let mut calendar = Calendar::new();
        calendar.push(ical_event);

        let ical_data = calendar.to_string();
        let event_url = format!("{}{}{}.ics", self.base_url, self.calendar_path, event.uid);

        let mut request = self
            .client
            .put(&event_url)
            .basic_auth(&self.username, Some(&self.password))
            .header("Content-Type", "text/calendar; charset=utf-8");

        if let Some(etag) = &event.etag {
            request = request.header("If-Match", etag);
        }

        let response = request.body(ical_data).send().await?;

        if response.status().is_success() {
            let etag = response
                .headers()
                .get("etag")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string());
            Ok(etag.unwrap_or_default())
        } else {
            Err(format!("Failed to update event: {}", response.status()).into())
        }
    }

    /// Delete an event from the CalDAV server
    pub async fn delete_event(
        &self,
        uid: &str,
        etag: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let event_url = format!("{}{}{}.ics", self.base_url, self.calendar_path, uid);

        let mut request = self
            .client
            .delete(&event_url)
            .basic_auth(&self.username, Some(&self.password));

        if let Some(etag) = etag {
            request = request.header("If-Match", etag);
        }

        let response = request.send().await?;

        if response.status().is_success() || response.status() == StatusCode::NO_CONTENT {
            Ok(())
        } else {
            Err(format!("Failed to delete event: {}", response.status()).into())
        }
    }

    /// Convert CalendarEvent to iCalendar Event
    fn event_to_ical(&self, event: &CalendarEvent) -> Event {
        let mut ical_event = Event::new();
        ical_event.uid(&event.uid);
        ical_event.summary(&event.title);
        ical_event.starts(event.start_time);
        ical_event.ends(event.end_time);

        if let Some(description) = &event.description {
            ical_event.description(description);
        }

        if let Some(location) = &event.location {
            ical_event.location(location);
        }

        ical_event.done()
    }

    /// Parse CalDAV XML response and extract calendar events
    fn parse_calendar_response(
        &self,
        xml: &str,
    ) -> Result<Vec<CalendarEvent>, Box<dyn std::error::Error + Send + Sync>> {
        let mut events = Vec::new();

        // Extract calendar data from XML response
        // Look for <C:calendar-data> or <calendar-data> tags
        let mut in_calendar_data = false;
        let mut current_ical = String::new();
        
        for line in xml.lines() {
            let trimmed = line.trim();
            
            // Start of calendar data
            if trimmed.contains("<C:calendar-data>") || trimmed.contains("<calendar-data>") {
                in_calendar_data = true;
                current_ical.clear();
                
                // Check if data is on same line
                if let Some(start) = trimmed.find(">") {
                    if let Some(end) = trimmed.find("</") {
                        current_ical = trimmed[start+1..end].to_string();
                        in_calendar_data = false;
                        
                        // Parse this iCalendar data
                        if let Ok(event) = self.parse_ical_event(&current_ical) {
                            events.push(event);
                        }
                    }
                }
                continue;
            }
            
            // End of calendar data
            if (trimmed.contains("</C:calendar-data>") || trimmed.contains("</calendar-data>")) && in_calendar_data {
                in_calendar_data = false;
                
                // Parse the collected iCalendar data
                if let Ok(event) = self.parse_ical_event(&current_ical) {
                    events.push(event);
                }
                current_ical.clear();
                continue;
            }
            
            // Collect calendar data
            if in_calendar_data {
                current_ical.push_str(trimmed);
                current_ical.push('\n');
            }
        }

        Ok(events)
    }

    /// Parse iCalendar string into CalendarEvent
    fn parse_ical_event(&self, ical_str: &str) -> Result<CalendarEvent, Box<dyn std::error::Error + Send + Sync>> {
        log::debug!("Parsing iCal event (first 200 chars): {}", ical_str.chars().take(200).collect::<String>());

        let mut uid = String::new();
        let mut title = String::from("Untitled Event");
        let mut description = None;
        let mut location = None;
        let mut start_time = chrono::Utc::now();
        let mut end_time = chrono::Utc::now();
        let mut attendees = Vec::new();
        let mut recurrence = None;
        let mut in_vevent = false;
        
        // Parse the iCalendar text manually for now
        for line in ical_str.lines() {
            let line = line.trim();
            
            if line == "BEGIN:VEVENT" {
                in_vevent = true;
                continue;
            }
            if line == "END:VEVENT" {
                in_vevent = false;
                continue;
            }
            
            if !in_vevent {
                continue;
            }
            
            if line.starts_with("UID:") {
                uid = line[4..].trim().to_string();
            } else if line.starts_with("SUMMARY:") {
                title = line[8..].trim().to_string();
            } else if line.starts_with("DESCRIPTION:") {
                description = Some(line[12..].trim().to_string());
            } else if line.starts_with("LOCATION:") {
                location = Some(line[9..].trim().to_string());
            } else if line.starts_with("DTSTART") {
                // Handle DTSTART;TZID=... or DTSTART:...
                if let Some(colon_pos) = line.find(':') {
                    let value = &line[colon_pos+1..];
                    if let Ok(dt) = self.parse_ical_datetime(value) {
                        start_time = dt;
                    }
                }
            } else if line.starts_with("DTEND") {
                // Handle DTEND;TZID=... or DTEND:...
                if let Some(colon_pos) = line.find(':') {
                    let value = &line[colon_pos+1..];
                    if let Ok(dt) = self.parse_ical_datetime(value) {
                        end_time = dt;
                    }
                }
            } else if line.starts_with("ATTENDEE") {
                if let Some(colon_pos) = line.find(':') {
                    attendees.push(line[colon_pos+1..].trim().to_string());
                }
            } else if line.starts_with("RRULE:") {
                recurrence = Some(line[6..].trim().to_string());
            }
        }
        
        if !uid.is_empty() {
            log::debug!("Parsed event: uid={}, title={}, start={}", uid, title, start_time);
            return Ok(CalendarEvent {
                uid,
                title,
                description,
                location,
                start_time,
                end_time,
                all_day: false,
                attendees,
                recurrence,
                etag: None,
            });
        }
        
        Err("No VEVENT found in iCalendar data".into())
    }
    
    /// Parse iCalendar datetime format
    fn parse_ical_datetime(&self, value: &str) -> Result<DateTime<Utc>, Box<dyn std::error::Error + Send + Sync>> {
        let value = value.trim();
        
        // Format: 20191106T023000 or 20191105T152000Z
        if value.ends_with('Z') {
            // UTC time
            let dt = chrono::NaiveDateTime::parse_from_str(&value[..value.len()-1], "%Y%m%dT%H%M%S")?;
            Ok(DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc))
        } else if value.contains('T') {
            // Local time (treat as UTC for now)
            let dt = chrono::NaiveDateTime::parse_from_str(value, "%Y%m%dT%H%M%S")?;
            Ok(DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc))
        } else {
            // Date only
            let date = chrono::NaiveDate::parse_from_str(value, "%Y%m%d")?;
            let datetime = date.and_hms_opt(0, 0, 0).unwrap();
            Ok(DateTime::<Utc>::from_naive_utc_and_offset(datetime, Utc))
        }
    }

    /// Sync events from database to CalDAV server
    pub async fn sync_to_server(
        &self,
        local_events: Vec<CalendarEvent>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        for event in local_events {
            match self.create_event(&event).await {
                Ok(etag) => {
                    log::info!("Synced event {} to CalDAV server (etag: {})", event.uid, etag);
                }
                Err(e) => {
                    log::error!("Failed to sync event {} to CalDAV: {}", event.uid, e);
                }
            }
        }
        Ok(())
    }
}
