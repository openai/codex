use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn timestamp_jitter_does_not_accumulate_but_mute_advances_the_clock() {
    let (_, mut track) = AudioTrack::new().unwrap();
    let start = Instant::now();
    // Empty samples exercise the real packetizer without requiring a bound network peer.
    for tick in 0..500 {
        let at = start + Duration::from_millis(tick * 20 + tick % 2);
        track.send(EncodedAudio { data: vec![], at }).await.unwrap();
    }
    assert_eq!(track.end, Some(start + Duration::from_millis(10_001)));
    let at = start + Duration::from_secs(30);
    track.send(EncodedAudio { data: vec![], at }).await.unwrap();
    assert_eq!(track.end, Some(at + Duration::from_millis(20)));
}
