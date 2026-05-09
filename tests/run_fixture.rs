#[path = "fixtures/team_demo_fixture.rs"]
mod team_demo_fixture;

use team_demo_fixture::TeamDemoFixture;

#[tokio::test]
async fn run_fixture_and_print() {
    let mut fixture = TeamDemoFixture::new().await;
    let result = fixture.run().await;
    println!("Proof status: {:?}", result.proof.status);
    println!("Failures: {:?}", result.proof.failures);
    println!("Gates: {:?}", result.proof.gates);
    println!("Worker results: {:?}", result.worker_results);
    println!(
        "Health report issues: {:?}",
        result.health_report.map(|r| r.issues_found)
    );

    for spec in &fixture.worker_specs {
        let hb = tokio::fs::read_to_string(&spec.heartbeat)
            .await
            .unwrap_or_default();
        println!("Heartbeat {}: {}", spec.name, hb);
        let wire_events = spec.inbox.parent().unwrap().join("wire-events.jsonl");
        if wire_events.exists() {
            let content = tokio::fs::read_to_string(&wire_events)
                .await
                .unwrap_or_default();
            println!(
                "Wire events {} ({} lines)",
                spec.name,
                content.lines().count()
            );
        } else {
            println!("Wire events {}: NOT FOUND", spec.name);
        }
    }

    println!("\n--- proof.json ---");
    let (json, md) = team_demo_fixture::read_proof_files(&fixture.state_dir);
    println!("{}", json);
    println!("\n--- proof.md ---");
    println!("{}", md);
}
