/// Redis slot management and mapping

use std::collections::HashMap;

/// Redis cluster slot mapping (0-16383)
#[derive(Debug, Clone)]
pub struct SlotMap {
    /// Maps slot number to backend ID
    slot_to_backend: HashMap<u16, String>,
    /// Maps backend ID to their slot ranges
    backend_to_slots: HashMap<String, Vec<SlotRange>>,
}

/// Represents a range of slots assigned to a backend
#[derive(Debug, Clone, PartialEq)]
pub struct SlotRange {
    pub start: u16,
    pub end: u16,
}

impl SlotRange {
    pub fn new(start: u16, end: u16) -> Self {
        Self { start, end }
    }

    pub fn contains(&self, slot: u16) -> bool {
        slot >= self.start && slot <= self.end
    }

    pub fn size(&self) -> u16 {
        self.end - self.start + 1
    }
}

impl SlotMap {
    pub fn new() -> Self {
        Self {
            slot_to_backend: HashMap::new(),
            backend_to_slots: HashMap::new(),
        }
    }

    /// Assign a range of slots to a backend
    pub fn assign_slots(&mut self, backend_id: String, slot_range: SlotRange) {
        // Remove any existing assignments for these slots
        for slot in slot_range.start..=slot_range.end {
            if let Some(old_backend) = self.slot_to_backend.remove(&slot) {
                // Remove slot from old backend's ranges
                if let Some(ranges) = self.backend_to_slots.get_mut(&old_backend) {
                    ranges.retain(|range| !range.contains(slot));
                    if ranges.is_empty() {
                        self.backend_to_slots.remove(&old_backend);
                    }
                }
            }
            self.slot_to_backend.insert(slot, backend_id.clone());
        }

        // Add range to backend's slot list
        self.backend_to_slots
            .entry(backend_id)
            .or_insert_with(Vec::new)
            .push(slot_range);
    }

    /// Get backend ID for a specific slot
    pub fn get_backend_for_slot(&self, slot: u16) -> Option<&String> {
        self.slot_to_backend.get(&slot)
    }

    /// Get all slot ranges for a backend
    pub fn get_slots_for_backend(&self, backend_id: &str) -> Option<&Vec<SlotRange>> {
        self.backend_to_slots.get(backend_id)
    }

    /// Remove all slots assigned to a backend
    pub fn remove_backend(&mut self, backend_id: &str) {
        if let Some(ranges) = self.backend_to_slots.remove(backend_id) {
            for range in ranges {
                for slot in range.start..=range.end {
                    self.slot_to_backend.remove(&slot);
                }
            }
        }
    }

    /// Check if all slots (0-16383) are assigned
    pub fn is_complete(&self) -> bool {
        self.slot_to_backend.len() == 16384
    }

    /// Get coverage statistics
    pub fn get_coverage(&self) -> SlotCoverage {
        let assigned_slots = self.slot_to_backend.len();
        let total_slots = 16384u16;
        let coverage_percentage = (assigned_slots as f64 / total_slots as f64) * 100.0;

        let mut backend_stats = HashMap::new();
        for (backend_id, ranges) in &self.backend_to_slots {
            let slot_count: u16 = ranges.iter().map(|r| r.size()).sum();
            backend_stats.insert(backend_id.clone(), slot_count);
        }

        SlotCoverage {
            assigned_slots: assigned_slots as u16,
            total_slots,
            coverage_percentage,
            backend_distribution: backend_stats,
        }
    }

    /// Parse CLUSTER NODES response to update slot mapping
    pub fn update_from_cluster_nodes(&mut self, cluster_nodes: &str) -> Result<(), SlotParseError> {
        // Clear existing mappings
        self.slot_to_backend.clear();
        self.backend_to_slots.clear();

        for line in cluster_nodes.lines() {
            if line.trim().is_empty() {
                continue;
            }

            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 8 {
                continue; // Invalid line
            }

            let _node_id = parts[0];
            let address_port = parts[1];
            let flags = parts[2];
            
            // Skip replica nodes and failed nodes
            if flags.contains("slave") || flags.contains("fail") {
                continue;
            }

            // Extract address for backend lookup
            let address = if let Some(at_pos) = address_port.find('@') {
                &address_port[..at_pos]
            } else {
                address_port
            };

            // Parse slot ranges (from index 8 onwards)
            for slot_spec in &parts[8..] {
                if slot_spec.contains('-') {
                    // Range like "0-5460"
                    let range_parts: Vec<&str> = slot_spec.split('-').collect();
                    if range_parts.len() == 2 {
                        if let (Ok(start), Ok(end)) = (range_parts[0].parse::<u16>(), range_parts[1].parse::<u16>()) {
                            self.assign_slots(address.to_string(), SlotRange::new(start, end));
                        }
                    }
                } else if let Ok(slot) = slot_spec.parse::<u16>() {
                    // Single slot
                    self.assign_slots(address.to_string(), SlotRange::new(slot, slot));
                }
            }
        }

        Ok(())
    }

    /// Get all backends that have slots assigned
    pub fn get_active_backends(&self) -> Vec<String> {
        self.backend_to_slots.keys().cloned().collect()
    }

    /// Find which slots are missing (not assigned to any backend)
    pub fn get_missing_slots(&self) -> Vec<u16> {
        let mut missing = Vec::new();
        for slot in 0..16384u16 {
            if !self.slot_to_backend.contains_key(&slot) {
                missing.push(slot);
            }
        }
        missing
    }
}

/// Statistics about slot coverage
#[derive(Debug, Clone)]
pub struct SlotCoverage {
    pub assigned_slots: u16,
    pub total_slots: u16,
    pub coverage_percentage: f64,
    pub backend_distribution: HashMap<String, u16>,
}

/// Error type for slot parsing
#[derive(Debug, thiserror::Error)]
pub enum SlotParseError {
    #[error("Invalid cluster nodes format")]
    InvalidFormat,
    #[error("Invalid slot range: {0}")]
    InvalidRange(String),
}

impl Default for SlotMap {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slot_assignment() {
        let mut slot_map = SlotMap::new();
        
        // Assign slots 0-100 to backend1
        slot_map.assign_slots("backend1".to_string(), SlotRange::new(0, 100));
        
        // Check slot assignment
        assert_eq!(slot_map.get_backend_for_slot(0), Some(&"backend1".to_string()));
        assert_eq!(slot_map.get_backend_for_slot(50), Some(&"backend1".to_string()));
        assert_eq!(slot_map.get_backend_for_slot(100), Some(&"backend1".to_string()));
        assert_eq!(slot_map.get_backend_for_slot(101), None);
        
        // Check backend slots
        let ranges = slot_map.get_slots_for_backend("backend1").unwrap();
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0], SlotRange::new(0, 100));
    }

    #[test]
    fn test_slot_coverage() {
        let mut slot_map = SlotMap::new();
        
        // Assign first half to backend1, second half to backend2
        slot_map.assign_slots("backend1".to_string(), SlotRange::new(0, 8191));
        slot_map.assign_slots("backend2".to_string(), SlotRange::new(8192, 16383));
        
        let coverage = slot_map.get_coverage();
        assert_eq!(coverage.assigned_slots, 16384);
        assert_eq!(coverage.coverage_percentage, 100.0);
        assert!(slot_map.is_complete());
        
        assert_eq!(coverage.backend_distribution.get("backend1"), Some(&8192));
        assert_eq!(coverage.backend_distribution.get("backend2"), Some(&8192));
    }

    #[test]
    fn test_cluster_nodes_parsing() {
        let mut slot_map = SlotMap::new();
        
        let cluster_nodes = r#"
07c37dfeb235213a872192d90877d0cd55635b91 127.0.0.1:30004@31004 slave e7d1eecce10fd6bb5eb35b9f99a514335d9ba9ca 0 1426238317239 4 connected
67ed2db8d677e59ec4a4cefb06858cf2a1a89fa1 127.0.0.1:30002@31002 master - 0 1426238316232 2 connected 5461-10922
292f8b365bb7edb5e285caf0b7e6ddc7265d2f4f 127.0.0.1:30003@31003 master - 0 1426238318243 3 connected 10923-16383
6ec23923021cf3ffec47632106199cb7f496ce01 127.0.0.1:30005@31005 slave 67ed2db8d677e59ec4a4cefb06858cf2a1a89fa1 0 1426238316232 5 connected
824fe116063bc5fcf9f4ffd395bc17adf3a3a6b2 127.0.0.1:30006@31006 slave 292f8b365bb7edb5e285caf0b7e6ddc7265d2f4f 0 1426238317741 6 connected
e7d1eecce10fd6bb5eb35b9f99a514335d9ba9ca 127.0.0.1:30001@31001 myself,master - 0 0 1 connected 0-5460
"#;

        slot_map.update_from_cluster_nodes(cluster_nodes).unwrap();
        
        // Verify slot assignments
        assert_eq!(slot_map.get_backend_for_slot(0), Some(&"127.0.0.1:30001".to_string()));
        assert_eq!(slot_map.get_backend_for_slot(5460), Some(&"127.0.0.1:30001".to_string()));
        assert_eq!(slot_map.get_backend_for_slot(5461), Some(&"127.0.0.1:30002".to_string()));
        assert_eq!(slot_map.get_backend_for_slot(10922), Some(&"127.0.0.1:30002".to_string()));
        assert_eq!(slot_map.get_backend_for_slot(10923), Some(&"127.0.0.1:30003".to_string()));
        assert_eq!(slot_map.get_backend_for_slot(16383), Some(&"127.0.0.1:30003".to_string()));
        
        assert!(slot_map.is_complete());
        
        let coverage = slot_map.get_coverage();
        assert_eq!(coverage.assigned_slots, 16384);
    }
}