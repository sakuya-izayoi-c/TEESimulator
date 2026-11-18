#include "../../keymint/early_boot_publication.h"

#include <cstdio>
#include <cstdlib>
#include <latch>
#include <thread>
#include <vector>

namespace {
// Inert IDs stand for the two levels. Test the publication policy, not key operations.
struct Pair {
  int ta_tee, ta_strongbox;
  bool operator==(const Pair&) const = default;
};
using Profiles = std::vector<Pair>;
void Check(bool ok, const char* what) {
  if (!ok) {
    std::fprintf(stderr, "FAIL: %s\n", what);
    std::exit(1);
  }
}

struct Fixture {
  std::mutex mu;
  teesim::EarlyBootPublication gate{mu};
  Profiles live{{1, 2}};
  bool boot_property = false;
  int publications = 0;
  std::vector<int> calls;

  Profiles Signal() { return gate.EndAndSnapshot([&] { return live; }); }
  int Commit(Profiles staged, int reject = -1) {
    return gate.Commit(live, std::move(staged), [&] { return boot_property; },
                       [&](int id) { calls.push_back(id); return id != reject; },
                       [&] { ++publications; });
  }
};

void PublicationFirst() {
  Fixture f;
  Check(f.Commit({{3, 4}, {5, 6}}) == 2 && f.calls.empty(), "pre-boot commit without replay");
  Check(f.Signal() == Profiles{{3, 4}, {5, 6}}, "later callback snapshots new live levels");
}

void CallbackFirst() {
  Fixture f;
  Profiles staged{{3, 4}, {5, 6}};
  std::latch snapshot_taken{1}, finish_callback{1};
  std::thread callback([&] {
    const auto snapshot = f.Signal();
    snapshot_taken.count_down();
    finish_callback.wait();  // Live fan-out still owns the old snapshot during publication.
    Check(snapshot == Profiles{{1, 2}}, "callback snapshot kept old generation");
  });
  snapshot_taken.wait();
  Check(f.Commit(staged) == 2, "publish after callback between staging and commit");
  Check(f.calls == std::vector<int>{3, 4, 5, 6} && f.live == staged,
        "every staged level restricted before publication");
  finish_callback.count_down();
  callback.join();
}

void ReplayFailure(int reject) {
  Fixture f;
  f.boot_property = true;  // Late load, no callback. First observation must survive a failed commit.
  Check(f.Commit({{3, 4}, {5, 6}}, reject) < 0, "reject whole generation on replay error");
  Check(f.live == Profiles{{1, 2}} && f.publications == 0, "old table and flags preserved");
  f.boot_property = false;
  f.calls.clear();
  Check(f.Commit({{7, 8}}) == 1 && f.calls == std::vector<int>{7, 8},
        "fresh retry replays both levels even if property no longer reads true");
  Check(f.publications == 1 && f.live == Profiles{{7, 8}}, "only retry is visible");
}

void ReaderDuringReplay() {
  Fixture f;
  f.Signal();
  std::latch last_level{1}, read_done{1};
  std::thread reader([&] {
    last_level.wait();
    std::lock_guard<std::mutex> lk(f.mu);
    Check(f.live == Profiles{{1, 2}} && f.publications == 0, "reader sees old config until ready");
    read_done.count_down();
  });
  auto apply = [&](int id) {
    if (id == 4) { last_level.count_down(); read_done.wait(); }
    return true;
  };
  Check(f.gate.Commit(f.live, Profiles{{3, 4}}, [] { return false; }, apply,
                      [&] { ++f.publications; }) == 1, "publish after replay");
  reader.join();
  Check(f.live == Profiles{{3, 4}} && f.publications == 1, "reader can now see new generation");
}
}  // namespace

int main() {
  PublicationFirst();
  CallbackFirst();
  ReplayFailure(5);  // TEE of the second profile
  ReplayFailure(6);  // StrongBox of the second profile, after every other level succeeded
  ReaderDuringReplay();
  std::puts("PASS early boot: both publication orders, late load, both failure levels, concurrent reader");
}
