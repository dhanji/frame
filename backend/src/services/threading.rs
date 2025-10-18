use std::collections::HashMap;
use serde::{Deserialize, Serialize};

/// Container for threading algorithm
#[derive(Debug, Clone)]
pub struct ThreadContainer {
    pub message_id: String,
    pub email_id: Option<i64>,
    pub parent: Option<String>,
    pub children: Vec<String>,
    pub subject: Option<String>,
    pub date: Option<chrono::DateTime<chrono::Utc>>,
}

/// Email data for threading
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadableEmail {
    pub id: i64,
    pub message_id: String,
    pub subject: String,
    pub in_reply_to: Option<String>,
    pub references: Vec<String>,
    pub date: chrono::DateTime<chrono::Utc>,
}

/// Thread tree node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadNode {
    pub email_id: Option<i64>,
    pub message_id: String,
    pub subject: Option<String>,
    pub date: Option<chrono::DateTime<chrono::Utc>>,
    pub children: Vec<ThreadNode>,
    pub is_dummy: bool,
}

/// Jamie Zawinski's threading algorithm implementation
pub struct JwzThreading {
    containers: HashMap<String, ThreadContainer>,
}

impl JwzThreading {
    pub fn new() -> Self {
        Self {
            containers: HashMap::new(),
        }
    }

    /// Thread a list of emails using JWZ algorithm
    pub fn thread_emails(&mut self, emails: Vec<ThreadableEmail>) -> Vec<ThreadNode> {
        // Step 1: Build the id_table
        self.build_id_table(&emails);

        // Step 2: Link messages using References and In-Reply-To
        self.link_messages(&emails);

        // Step 3: Find root messages (messages with no parent)
        let roots = self.find_roots();

        // Step 4: Prune empty containers
        let pruned_roots = self.prune_empty_containers(roots);

        // Step 5: Group by subject
        let grouped_roots = self.group_by_subject(pruned_roots);

        // Step 6: Build thread tree
        self.build_thread_tree(grouped_roots)
    }

    /// Step 1: Build ID table from message IDs
    fn build_id_table(&mut self, emails: &[ThreadableEmail]) {
        for email in emails {
            // Create container for this message if it doesn't exist
            self.containers
                .entry(email.message_id.clone())
                .or_insert_with(|| ThreadContainer {
                    message_id: email.message_id.clone(),
                    email_id: Some(email.id),
                    parent: None,
                    children: Vec::new(),
                    subject: Some(email.subject.clone()),
                    date: Some(email.date),
                });

            // Create containers for all referenced messages
            for reference in &email.references {
                self.containers
                    .entry(reference.clone())
                    .or_insert_with(|| ThreadContainer {
                        message_id: reference.clone(),
                        email_id: None,
                        parent: None,
                        children: Vec::new(),
                        subject: None,
                        date: None,
                    });
            }

            // Create container for In-Reply-To if present
            if let Some(in_reply_to) = &email.in_reply_to {
                self.containers
                    .entry(in_reply_to.clone())
                    .or_insert_with(|| ThreadContainer {
                        message_id: in_reply_to.clone(),
                        email_id: None,
                        parent: None,
                        children: Vec::new(),
                        subject: None,
                        date: None,
                    });
            }
        }
    }

    /// Step 2: Link messages using References and In-Reply-To headers
    fn link_messages(&mut self, emails: &[ThreadableEmail]) {
        for email in emails {
            let message_id = email.message_id.clone();

            // Build reference chain
            let mut refs = email.references.clone();

            // Add In-Reply-To to references if not already present
            if let Some(in_reply_to) = &email.in_reply_to {
                if !refs.contains(in_reply_to) {
                    refs.push(in_reply_to.clone());
                }
            }

            // Link each reference to the next in the chain
            for i in 0..refs.len() {
                let current_ref = refs[i].clone();

                // Link to next reference or to the message itself
                let child_id = if i + 1 < refs.len() {
                    refs[i + 1].clone()
                } else {
                    message_id.clone()
                };

                // Set parent-child relationship
                if let Some(parent_container) = self.containers.get_mut(&current_ref) {
                    if !parent_container.children.contains(&child_id) {
                        parent_container.children.push(child_id.clone());
                    }
                }

                if let Some(child_container) = self.containers.get_mut(&child_id) {
                    if child_container.parent.is_none() {
                        child_container.parent = Some(current_ref.clone());
                    }
                }
            }
        }
    }

    /// Step 3: Find root messages (messages with no parent)
    fn find_roots(&self) -> Vec<String> {
        self.containers
            .values()
            .filter(|c| c.parent.is_none())
            .map(|c| c.message_id.clone())
            .collect()
    }

    /// Step 4: Prune empty containers (dummy messages with no children)
    fn prune_empty_containers(&mut self, roots: Vec<String>) -> Vec<String> {
        let mut pruned_roots = Vec::new();

        for root_id in roots {
            let should_promote_children = {
                if let Some(container) = self.containers.get(&root_id) {
                // If container has no email and no children, skip it
                if container.email_id.is_none() && container.children.is_empty() {
                    continue;
                }

                // If container has no email but has children, promote children to root
                if container.email_id.is_none() && !container.children.is_empty() {
                        Some(container.children.clone())
                } else {
                        None
                    }
                } else {
                    None
                }
            };
            
            if let Some(children) = should_promote_children {
                for child_id in children {
                    if let Some(child) = self.containers.get_mut(&child_id) {
                        child.parent = None;
                    }
                    pruned_roots.push(child_id);
            }
            } else if self.containers.contains_key(&root_id) {
                pruned_roots.push(root_id);
            }
        }

        pruned_roots
    }

    /// Step 5: Group root messages by subject
    fn group_by_subject(&mut self, roots: Vec<String>) -> Vec<String> {
        let mut subject_table: HashMap<String, String> = HashMap::new();
        let mut grouped_roots = Vec::new();

        for root_id in roots {
            if let Some(container) = self.containers.get(&root_id) {
                if let Some(subject) = &container.subject {
                    let normalized_subject = self.normalize_subject(subject);

                    if let Some(existing_root_id) = subject_table.get(&normalized_subject) {
                        // Merge this thread with existing thread
                        if let Some(existing_root) = self.containers.get_mut(existing_root_id) {
                            if !existing_root.children.contains(&root_id) {
                                existing_root.children.push(root_id.clone());
                            }
                        }

                        if let Some(current_container) = self.containers.get_mut(&root_id) {
                            current_container.parent = Some(existing_root_id.clone());
                        }
                    } else {
                        // New subject, add to table
                        subject_table.insert(normalized_subject, root_id.clone());
                        grouped_roots.push(root_id);
                    }
                } else {
                    // No subject, keep as separate root
                    grouped_roots.push(root_id);
                }
            }
        }

        grouped_roots
    }

    /// Normalize subject by removing Re:, Fwd:, etc.
    fn normalize_subject(&self, subject: &str) -> String {
        let mut normalized = subject.trim().to_lowercase();

        // Remove common prefixes
        loop {
            let original = normalized.clone();

            // Remove Re:
            if let Some(stripped) = normalized.strip_prefix("re:") {
                normalized = stripped.trim().to_string();
            }

            // Remove Fwd:
            if let Some(stripped) = normalized.strip_prefix("fwd:") {
                normalized = stripped.trim().to_string();
            }

            // Remove Fw:
            if let Some(stripped) = normalized.strip_prefix("fw:") {
                normalized = stripped.trim().to_string();
            }

            // Remove [list-name]
            if normalized.starts_with('[') {
                if let Some(end_bracket) = normalized.find(']') {
                    normalized = normalized[end_bracket + 1..].trim().to_string();
                }
            }

            // If nothing changed, we're done
            if normalized == original {
                break;
            }
        }

        normalized
    }

    /// Step 6: Build thread tree from roots
    fn build_thread_tree(&self, roots: Vec<String>) -> Vec<ThreadNode> {
        let mut trees = Vec::new();

        for root_id in roots {
            if let Some(node) = self.build_node(&root_id) {
                trees.push(node);
            }
        }

        // Sort by date (most recent first)
        trees.sort_by(|a, b| {
            let date_a = a.date.unwrap_or_else(chrono::Utc::now);
            let date_b = b.date.unwrap_or_else(chrono::Utc::now);
            date_b.cmp(&date_a)
        });

        trees
    }

    /// Build a thread node recursively
    fn build_node(&self, message_id: &str) -> Option<ThreadNode> {
        let container = self.containers.get(message_id)?;

        let mut children = Vec::new();
        for child_id in &container.children {
            if let Some(child_node) = self.build_node(child_id) {
                children.push(child_node);
            }
        }

        // Sort children by date
        children.sort_by(|a, b| {
            let date_a = a.date.unwrap_or_else(chrono::Utc::now);
            let date_b = b.date.unwrap_or_else(chrono::Utc::now);
            date_a.cmp(&date_b)
        });

        Some(ThreadNode {
            email_id: container.email_id,
            message_id: container.message_id.clone(),
            subject: container.subject.clone(),
            date: container.date,
            children,
            is_dummy: container.email_id.is_none(),
        })
    }
}

impl Default for JwzThreading {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_normalize_subject() {
        let threading = JwzThreading::new();

        assert_eq!(threading.normalize_subject("Re: Hello"), "hello");
        assert_eq!(threading.normalize_subject("Fwd: Re: Hello"), "hello");
        assert_eq!(threading.normalize_subject("[List] Hello"), "hello");
        assert_eq!(threading.normalize_subject("Re: [List] Fwd: Hello"), "hello");
    }

    #[test]
    fn test_simple_thread() {
        let mut threading = JwzThreading::new();

        let emails = vec![
            ThreadableEmail {
                id: 1,
                message_id: "msg1".to_string(),
                subject: "Hello".to_string(),
                in_reply_to: None,
                references: vec![],
                date: Utc::now(),
            },
            ThreadableEmail {
                id: 2,
                message_id: "msg2".to_string(),
                subject: "Re: Hello".to_string(),
                in_reply_to: Some("msg1".to_string()),
                references: vec!["msg1".to_string()],
                date: Utc::now(),
            },
        ];

        let threads = threading.thread_emails(emails);

        assert_eq!(threads.len(), 1);
        assert_eq!(threads[0].children.len(), 1);
    }
}
