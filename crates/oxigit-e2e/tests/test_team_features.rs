mod harness;

use harness::*;

/// Helper: simulate upgrading a user to Team plan via Stripe webhook.
async fn upgrade_to_team(client: &TestClient, base_url: &str, user_id: &str, secret: &str) {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    let payload = format!(
        r#"{{"type":"checkout.session.completed","data":{{"object":{{"client_reference_id":"{}","customer":"cus_team_{}","subscription":"sub_team_{}","metadata":{{"plan":"team"}}}}}}}}"#,
        user_id, user_id, user_id
    );
    let timestamp = "1234567890";
    let signed_payload = format!("{}.{}", timestamp, payload);
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(signed_payload.as_bytes());
    let sig = hex::encode(mac.finalize().into_bytes());
    let signature = format!("t={},v1={}", timestamp, sig);

    let resp = client
        .client
        .post(format!("{}/api/stripe/webhook", base_url))
        .header("stripe-signature", &signature)
        .header("content-type", "application/json")
        .body(payload)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200, "team webhook should succeed");
}

// ---------------------------------------------------------------------------
// Org creation
// ---------------------------------------------------------------------------

/// Test: authenticated user can create an organization.
#[tokio::test]
async fn create_org_succeeds() {
    let server = TestServer::start().await;
    let client = server.client();

    client.register("alice", "alice@test.com", "password123").await;
    client.login("alice", "password123").await;

    let resp = client.create_org("myorg", "My Organization").await;
    assert!(
        resp.status().is_success() || resp.status().is_redirection(),
        "org creation should succeed, got {}",
        resp.status()
    );
}

/// Test: list_my_orgs returns the created organization with owner role.
#[tokio::test]
async fn list_my_orgs_shows_created_org() {
    let server = TestServer::start().await;
    let client = server.client();

    client.register("alice", "alice@test.com", "password123").await;
    client.login("alice", "password123").await;
    client.create_org("devteam", "Dev Team").await;

    let resp = client.list_my_orgs().await;
    let body = resp.text().await.unwrap();

    assert!(
        body.contains("devteam"),
        "list_my_orgs should include the org slug: {body}"
    );
    assert!(
        body.contains("owner"),
        "creator should have owner role: {body}"
    );
}

// ---------------------------------------------------------------------------
// Org switching
// ---------------------------------------------------------------------------

/// Test: member can switch to an organization they belong to.
#[tokio::test]
async fn switch_org_works_for_member() {
    let server = TestServer::start().await;
    let client = server.client();

    client.register("alice", "alice@test.com", "password123").await;
    client.login("alice", "password123").await;
    client.create_org("myorg", "My Org").await;

    let resp = client.switch_org("myorg").await;
    // switch_org redirects to /repos on success
    assert!(
        resp.status().is_success() || resp.status().is_redirection(),
        "switch_org should succeed for org member, got {}",
        resp.status()
    );
}

/// Test: non-member cannot switch to an organization.
#[tokio::test]
async fn switch_org_blocked_for_non_member() {
    let server = TestServer::start().await;

    let alice = server.client();
    alice.register("alice", "alice@test.com", "password123").await;
    alice.login("alice", "password123").await;
    alice.create_org("secretorg", "Secret Org").await;

    let bob = server.client();
    bob.register("bob", "bob@test.com", "password123").await;
    bob.login("bob", "password123").await;

    let resp = bob.switch_org("secretorg").await;
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("Not a member"),
        "non-member should be denied org switch: {body}"
    );
}

// ---------------------------------------------------------------------------
// Org settings access
// ---------------------------------------------------------------------------

/// Test: free user is blocked from org settings (team management).
#[tokio::test]
async fn free_user_blocked_from_org_settings() {
    let server = TestServer::start().await;
    let client = server.client();

    client.register("alice", "alice@test.com", "password123").await;
    client.login("alice", "password123").await;
    client.create_org("myorg", "My Org").await;

    // Try to get org settings via server function — should be blocked on Free plan
    let resp = client.get_org_settings("myorg").await;
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("Team plan") || body.contains("Upgrade"),
        "Org settings should show upgrade prompt for free users: {body}"
    );
}

/// Test: Team user can access org settings and see members.
#[tokio::test]
async fn team_user_can_access_org_settings() {
    let secret = "whsec_team_settings_test";
    let server = TestServer::start_with_stripe(secret).await;
    let client = server.client();

    client.register("alice", "alice@test.com", "password123").await;
    upgrade_to_team(&client, &client.base_url.clone(), "1", secret).await;
    client.login("alice", "password123").await;
    client.create_org("myorg", "My Org").await;

    let resp = client.get_org_settings("myorg").await;
    let body = resp.text().await.unwrap();
    assert!(
        !body.contains("Team plan") && !body.contains("Upgrade"),
        "Team user should access org settings without upgrade prompt: {body}"
    );
    assert!(
        body.contains("alice"),
        "Org settings should show creator as member: {body}"
    );
    assert!(
        body.contains("owner"),
        "Creator should have owner role: {body}"
    );
}

/// Test: non-member is blocked from accessing org settings even with Team plan.
#[tokio::test]
async fn non_member_blocked_from_org_settings() {
    let secret = "whsec_team_non_member_test";
    let server = TestServer::start_with_stripe(secret).await;

    let alice = server.client();
    alice.register("alice", "alice@test.com", "password123").await;
    upgrade_to_team(&alice, &alice.base_url.clone(), "1", secret).await;
    alice.login("alice", "password123").await;
    alice.create_org("secretorg", "Secret Org").await;

    let bob = server.client();
    bob.register("bob", "bob@test.com", "password123").await;
    upgrade_to_team(&bob, &bob.base_url.clone(), "2", secret).await;
    bob.login("bob", "password123").await;

    let resp = bob.get_org_settings("secretorg").await;
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("Not a member"),
        "non-member should be denied org settings: {body}"
    );
}

// ---------------------------------------------------------------------------
// Member management
// ---------------------------------------------------------------------------

/// Test: owner can add a member and they appear in org settings.
#[tokio::test]
async fn owner_can_add_member() {
    let secret = "whsec_team_add_member_test";
    let server = TestServer::start_with_stripe(secret).await;

    let alice = server.client();
    alice.register("alice", "alice@test.com", "password123").await;
    upgrade_to_team(&alice, &alice.base_url.clone(), "1", secret).await;
    alice.login("alice", "password123").await;
    alice.create_org("devteam", "Dev Team").await;

    // Register bob so he can be added
    let bob = server.client();
    bob.register("bob", "bob@test.com", "password123").await;

    // Alice adds bob as member
    let resp = alice.add_org_member("devteam", "bob", "member").await;
    let body = resp.text().await.unwrap();
    assert!(
        !body.contains("error") || body.contains("Ok"),
        "adding member should succeed: {body}"
    );

    // Verify bob appears in settings
    let resp = alice.get_org_settings("devteam").await;
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("bob"),
        "added member should appear in org settings: {body}"
    );
}

/// Test: owner can add a member with admin role.
#[tokio::test]
async fn owner_can_add_admin() {
    let secret = "whsec_team_add_admin_test";
    let server = TestServer::start_with_stripe(secret).await;

    let alice = server.client();
    alice.register("alice", "alice@test.com", "password123").await;
    upgrade_to_team(&alice, &alice.base_url.clone(), "1", secret).await;
    alice.login("alice", "password123").await;
    alice.create_org("devteam", "Dev Team").await;

    let bob = server.client();
    bob.register("bob", "bob@test.com", "password123").await;

    alice.add_org_member("devteam", "bob", "admin").await;

    let resp = alice.get_org_settings("devteam").await;
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("bob"),
        "admin should appear in org settings: {body}"
    );
    assert!(
        body.contains("admin"),
        "member should have admin role: {body}"
    );
}

/// Test: owner can remove a member from the organization.
#[tokio::test]
async fn owner_can_remove_member() {
    let secret = "whsec_team_remove_test";
    let server = TestServer::start_with_stripe(secret).await;

    let alice = server.client();
    alice.register("alice", "alice@test.com", "password123").await;
    upgrade_to_team(&alice, &alice.base_url.clone(), "1", secret).await;
    alice.login("alice", "password123").await;
    alice.create_org("devteam", "Dev Team").await;

    let bob = server.client();
    bob.register("bob", "bob@test.com", "password123").await;

    // Add bob (user_id = 2)
    alice.add_org_member("devteam", "bob", "member").await;

    // Verify bob is a member
    let resp = alice.get_org_settings("devteam").await;
    let body = resp.text().await.unwrap();
    assert!(body.contains("bob"), "bob should be in org before removal");

    // Remove bob (user_id = 2)
    let resp = alice.remove_org_member("devteam", 2).await;
    let body = resp.text().await.unwrap();
    assert!(
        !body.contains("error") && !body.contains("Only org owners"),
        "removing member should succeed: {body}"
    );

    // Verify bob is gone
    let resp = alice.get_org_settings("devteam").await;
    let body = resp.text().await.unwrap();
    assert!(
        !body.contains("bob"),
        "removed member should not appear in settings: {body}"
    );
}

/// Test: owner cannot remove themselves from the organization.
#[tokio::test]
async fn owner_cannot_remove_self() {
    let secret = "whsec_team_remove_self_test";
    let server = TestServer::start_with_stripe(secret).await;

    let alice = server.client();
    alice.register("alice", "alice@test.com", "password123").await;
    upgrade_to_team(&alice, &alice.base_url.clone(), "1", secret).await;
    alice.login("alice", "password123").await;
    alice.create_org("devteam", "Dev Team").await;

    // Try to remove self (user_id = 1)
    let resp = alice.remove_org_member("devteam", 1).await;
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("Cannot remove yourself"),
        "owner should not be able to remove self: {body}"
    );
}

/// Test: non-owner member cannot add other members.
#[tokio::test]
async fn non_owner_cannot_add_member() {
    let secret = "whsec_team_non_owner_add_test";
    let server = TestServer::start_with_stripe(secret).await;

    let alice = server.client();
    alice.register("alice", "alice@test.com", "password123").await;
    upgrade_to_team(&alice, &alice.base_url.clone(), "1", secret).await;
    alice.login("alice", "password123").await;
    alice.create_org("devteam", "Dev Team").await;

    let bob = server.client();
    bob.register("bob", "bob@test.com", "password123").await;
    upgrade_to_team(&bob, &bob.base_url.clone(), "2", secret).await;

    // Alice adds bob as regular member
    alice.add_org_member("devteam", "bob", "member").await;

    // Register charlie
    let charlie = server.client();
    charlie.register("charlie", "charlie@test.com", "password123").await;

    // Bob (member, not owner) tries to add charlie
    bob.login("bob", "password123").await;
    let resp = bob.add_org_member("devteam", "charlie", "member").await;
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("Only org owners"),
        "non-owner should be blocked from adding members: {body}"
    );
}

/// Test: non-owner member cannot remove other members.
#[tokio::test]
async fn non_owner_cannot_remove_member() {
    let secret = "whsec_team_non_owner_rm_test";
    let server = TestServer::start_with_stripe(secret).await;

    let alice = server.client();
    alice.register("alice", "alice@test.com", "password123").await;
    upgrade_to_team(&alice, &alice.base_url.clone(), "1", secret).await;
    alice.login("alice", "password123").await;
    alice.create_org("devteam", "Dev Team").await;

    let bob = server.client();
    bob.register("bob", "bob@test.com", "password123").await;
    upgrade_to_team(&bob, &bob.base_url.clone(), "2", secret).await;

    let charlie = server.client();
    charlie.register("charlie", "charlie@test.com", "password123").await;

    // Alice adds bob and charlie
    alice.add_org_member("devteam", "bob", "member").await;
    alice.add_org_member("devteam", "charlie", "member").await;

    // Bob (member) tries to remove charlie (user_id = 3)
    bob.login("bob", "password123").await;
    let resp = bob.remove_org_member("devteam", 3).await;
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("Only org owners"),
        "non-owner should be blocked from removing members: {body}"
    );
}

/// Test: free user is blocked from adding members (entitlement check).
#[tokio::test]
async fn free_user_blocked_from_add_member() {
    let server = TestServer::start().await;
    let client = server.client();

    client.register("alice", "alice@test.com", "password123").await;
    client.login("alice", "password123").await;
    client.create_org("myorg", "My Org").await;

    // Register bob
    let bob = server.client();
    bob.register("bob", "bob@test.com", "password123").await;

    // Alice (free plan) tries to add bob
    let resp = client.add_org_member("myorg", "bob", "member").await;
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("Team plan") || body.contains("Upgrade"),
        "free user should be blocked from adding members: {body}"
    );
}

/// Test: free user is blocked from removing members (entitlement check).
#[tokio::test]
async fn free_user_blocked_from_remove_member() {
    let server = TestServer::start().await;
    let client = server.client();

    client.register("alice", "alice@test.com", "password123").await;
    client.login("alice", "password123").await;
    client.create_org("myorg", "My Org").await;

    // Try to remove user_id=1 (self) — entitlement check should fail before self-check
    let resp = client.remove_org_member("myorg", 1).await;
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("Team plan") || body.contains("Upgrade"),
        "free user should be blocked from removing members: {body}"
    );
}

/// Test: adding a nonexistent user returns an error.
#[tokio::test]
async fn add_nonexistent_user_fails() {
    let secret = "whsec_team_nonexist_test";
    let server = TestServer::start_with_stripe(secret).await;

    let alice = server.client();
    alice.register("alice", "alice@test.com", "password123").await;
    upgrade_to_team(&alice, &alice.base_url.clone(), "1", secret).await;
    alice.login("alice", "password123").await;
    alice.create_org("devteam", "Dev Team").await;

    let resp = alice.add_org_member("devteam", "nobody", "member").await;
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("User not found") || body.contains("not found"),
        "adding nonexistent user should fail: {body}"
    );
}

/// Test: added member can switch to and list the organization.
#[tokio::test]
async fn added_member_can_list_and_switch_org() {
    let secret = "whsec_team_member_access_test";
    let server = TestServer::start_with_stripe(secret).await;

    let alice = server.client();
    alice.register("alice", "alice@test.com", "password123").await;
    upgrade_to_team(&alice, &alice.base_url.clone(), "1", secret).await;
    alice.login("alice", "password123").await;
    alice.create_org("devteam", "Dev Team").await;

    let bob = server.client();
    bob.register("bob", "bob@test.com", "password123").await;
    upgrade_to_team(&bob, &bob.base_url.clone(), "2", secret).await;

    // Alice adds bob
    alice.add_org_member("devteam", "bob", "member").await;

    // Bob can list and see the org
    bob.login("bob", "password123").await;
    let resp = bob.list_my_orgs().await;
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("devteam"),
        "added member should see org in list: {body}"
    );

    // Bob can switch to the org
    let resp = bob.switch_org("devteam").await;
    assert!(
        resp.status().is_success() || resp.status().is_redirection(),
        "added member should be able to switch to org, got {}",
        resp.status()
    );
}

/// Test: org settings page (HTML) shows member management for team users.
#[tokio::test]
async fn org_settings_page_shows_members() {
    let secret = "whsec_team_page_test";
    let server = TestServer::start_with_stripe(secret).await;
    let client = server.client();

    client.register("alice", "alice@test.com", "password123").await;
    upgrade_to_team(&client, &client.base_url.clone(), "1", secret).await;
    client.login("alice", "password123").await;
    client.create_org("myorg", "My Org").await;

    // Fetch the actual settings page (HTML)
    let resp = client
        .client
        .get(format!("{}/orgs/myorg/settings", client.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    let body = resp.text().await.unwrap();

    assert!(
        body.contains("Settings") || body.contains("Members"),
        "org settings page should show member management: {body}"
    );
}
