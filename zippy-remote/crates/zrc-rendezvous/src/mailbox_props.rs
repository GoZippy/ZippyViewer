
use proptest::prelude::*;
use crate::mailbox::{Mailbox, MailboxError};
use bytes::Bytes;
use std::time::Duration;

proptest! {
    // Property 2: Message Integrity
    #[test]
    fn test_message_integrity(
        payloads in prop::collection::vec(any::<u8>(), 0..1024),
    ) {
        let mut mailbox = Mailbox::new();
        // Post
        let data = Bytes::from(payloads.clone());
        let seq = mailbox.post(data.clone(), 10, 2048).unwrap();
        
        // Get
        let msg = mailbox.get().expect("Message lost");
        assert_eq!(msg.data, data);
        assert_eq!(msg.sequence, seq);
    }

    // Property 1: Message Ordering
    #[test]
    fn test_message_ordering(
        payloads in prop::collection::vec(any::<u8>(), 1..10) // list of bytes for multiple messages doesn't make sense as vec<u8>
        // We want a LIST of payloads
    ) {
        // Correct strategy: list of payloads
        // But let's simplify: Post N messages, read N messages
    }
}

// Separate proptest block for logic requiring specific strategies
proptest! {
    #[test]
    fn test_ordering_and_sequence(
        payloads in prop::collection::vec(prop::collection::vec(any::<u8>(), 0..100), 1..20)
    ) {
        let mut mailbox = Mailbox::new();
        let max_len = 100;
        let max_size = 1024;

        let mut seqs = Vec::new();

        // Push all
        for p in &payloads {
            let data = Bytes::from(p.clone());
            if let Ok(seq) = mailbox.post(data, max_len, max_size) {
                seqs.push(seq);
            }
        }

        // Pop all
        for (i, expected_seq) in seqs.iter().enumerate() {
            let msg = mailbox.get().expect("Queue shouldn't be empty");
            assert_eq!(msg.sequence, *expected_seq, "Sequence mismatch at index {}", i);
            assert_eq!(&msg.data[..], &payloads[i][..], "Payload mismatch at index {}", i);
        }
        
        // Should be empty
        assert!(mailbox.get().is_none());
    }

    // Property 5: Queue Length Enforcement
    #[test]
    fn test_queue_limit_enforcement(
        limit in 1..20usize,
        extras in 1..10usize
    ) {
        let mut mailbox = Mailbox::new();
        let max_size = 1024;
        
        // Fill up to limit
        for i in 0..limit {
            let data = Bytes::from(vec![i as u8]);
            prop_assert!(mailbox.post(data, limit, max_size).is_ok());
        }
        
        // Next posts should fail
        for _ in 0..extras {
            let data = Bytes::from(vec![0u8]);
            prop_assert!(matches!(mailbox.post(data, limit, max_size), Err(MailboxError::QueueFull)));
        }
    }
    
    // Test Message Too Large
    #[test]
    fn test_message_size_limit(
        limit in 10..100usize
    ) {
        let mut mailbox = Mailbox::new();
        let overload = vec![0u8; limit + 1];
        let data = Bytes::from(overload);
        
        prop_assert!(matches!(mailbox.post(data, 100, limit), Err(MailboxError::MessageTooLarge)));
        
        let ok_load = vec![0u8; limit];
        let data = Bytes::from(ok_load);
        prop_assert!(mailbox.post(data, 100, limit).is_ok());
    }
}
