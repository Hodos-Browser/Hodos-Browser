// beta.3 — the prompt queue must never strand a prompt that a stale close hid.
// TICKET_connect_prompts_arrive_after_approval_and_hang.md
//
// 📏 zanaadu.com, 2026-09-21. One Approve click sends "answer" and then "close". Between
// the two, a new prompt took the shared overlay; the close then hid it. The queue still
// believed it was on screen, so every later prompt waited behind an invisible one until it
// timed out 10 minutes later.
//
// These tests drive PendingRequestManager directly, because the live race needs calls in
// flight during a ~150 ms window and reproduced once in two tries — no negative control is
// possible live. Here it is deterministic.
//
// ⛔ NEGATIVE CONTROL: make `takeHiddenShownPrompt` return false unconditionally and ONLY
// `StaleCloseHidNewerPrompt_ItIsPutBack` goes red. The other three stay green — they assert
// what the fix must NOT do (resurrect an answered prompt, or steal the normal queue's job).

#include "core/PendingAuthRequest.h"

#include <gtest/gtest.h>

#include <vector>

namespace {

// The singleton outlives each test — give every test its own domain.
PendingAuthRequest Prompt(const std::string& domain) {
    PendingAuthRequest r;
    r.domain = domain;
    r.overlayType = "manifest_connect_bundle";
    r.createdAt = std::chrono::steady_clock::now();
    return r;
}

void Drop(const std::string& id) {
    PendingAuthRequest gone;
    PendingRequestManager::GetInstance().popRequest(id, gone);
}

// ⛔ RAII cleanup, not a trailing Drop(). A failing ASSERT returns from the test early, and a
// prompt left marked "shown" in the singleton makes the NEXT test's addPromptRequest queue
// instead of show. 📏 Found by running the negative control: with the fix disabled, three
// tests went red instead of one — two of them for this leak, not for the fix being off.
// Popping an id twice is harmless, so a scenario step may Drop() an id this also owns.
struct Owned {
    std::vector<std::string> ids;
    std::string add(const std::string& id) { ids.push_back(id); return id; }
    ~Owned() { for (const auto& id : ids) Drop(id); }
};

}  // namespace

TEST(PromptQueueHiddenPrompt, StaleCloseHidNewerPrompt_ItIsPutBack) {
    auto& q = PendingRequestManager::GetInstance();
    Owned own;
    bool showNow = false;
    int fromSite = 0;

    // A is on screen.
    const std::string a = own.add(q.addPromptRequest(Prompt("hidden-a.test"), showNow, fromSite));
    ASSERT_TRUE(showNow);

    // The approval drain pops A before the click's `answer` message arrives...
    Drop(a);
    // ...so B, arriving now, takes the overlay.
    const std::string b = own.add(q.addPromptRequest(Prompt("hidden-a.test"), showNow, fromSite));
    ASSERT_TRUE(showNow) << "B should have been posted straight to the overlay";

    // Then the click's answer (for A) and its close land. The close hid B.
    q.noteAnswered(a);

    PendingAuthRequest back;
    int waiting = -1;
    ASSERT_TRUE(q.takeHiddenShownPrompt(back, waiting))
        << "B is marked on screen, still pending, and nobody can see it — it must be re-shown";
    EXPECT_EQ(back.requestId, b);
}

TEST(PromptQueueHiddenPrompt, OwnClose_NeverReshowsTheAnsweredPrompt) {
    // The normal case: the user answers A and its close arrives BEFORE A is popped.
    // Re-showing A here would put an answered prompt back on screen — a ghost.
    auto& q = PendingRequestManager::GetInstance();
    Owned own;
    bool showNow = false;
    int fromSite = 0;
    const std::string a = own.add(q.addPromptRequest(Prompt("hidden-b.test"), showNow, fromSite));
    ASSERT_TRUE(showNow);

    q.noteAnswered(a);

    PendingAuthRequest back;
    int waiting = -1;
    EXPECT_FALSE(q.takeHiddenShownPrompt(back, waiting));
}

TEST(PromptQueueHiddenPrompt, NothingOnScreen_NothingToPutBack) {
    auto& q = PendingRequestManager::GetInstance();
    q.noteAnswered("req-none");
    PendingAuthRequest back;
    int waiting = -1;
    EXPECT_FALSE(q.takeHiddenShownPrompt(back, waiting));
}

TEST(PromptQueueHiddenPrompt, QueuedButNeverShown_IsLeftToTheNormalQueue) {
    // A prompt that is merely WAITING (not marked shown) is the normal queue's job
    // (takeNextQueuedPrompt). Re-show must only ever touch one marked on screen.
    auto& q = PendingRequestManager::GetInstance();
    Owned own;
    bool showNow = false;
    int fromSite = 0;
    const std::string a = own.add(q.addPromptRequest(Prompt("hidden-c.test"), showNow, fromSite));
    ASSERT_TRUE(showNow);
    own.add(q.addPromptRequest(Prompt("hidden-c.test"), showNow, fromSite));
    ASSERT_FALSE(showNow) << "B should queue behind A";

    q.noteAnswered(a);
    Drop(a);  // A answered and gone; B is still only queued

    PendingAuthRequest back;
    int waiting = -1;
    EXPECT_FALSE(q.takeHiddenShownPrompt(back, waiting));
}
