#pragma once

#include <mutex>

namespace teesim {

// The callback snapshot and publication use the same mutex as routing readers. Only the
// control thread prepares/commits profiles; the callback never needs to inspect staging.
class EarlyBootPublication {
 public:
  explicit EarlyBootPublication(std::mutex& cfg_mu) : cfg_mu_(cfg_mu) {}

  template <class Snapshot>
  auto EndAndSnapshot(Snapshot snapshot_locked) {
    std::lock_guard<std::mutex> lock(cfg_mu_);
    ended_ = true;
    return snapshot_locked();
  }

  template <class Profiles, class BootCompleted, class EndTa, class OnPublish>
  int Commit(Profiles& live, Profiles next, BootCompleted boot_completed, EndTa end_ta,
             OnPublish publish_flags_locked) {
    std::unique_lock<std::mutex> lock(cfg_mu_);
    ended_ = ended_ || boot_completed();  // A late-load observation is one-way too.
    if (ended_) {
      // No path back to early boot. Replay on the private candidate without nesting the
      // Rust TA lock inside cfg_mu_, then publish only if both levels of every profile agreed.
      lock.unlock();
      for (const auto& p : next) {
        if (!end_ta(p.ta_tee) || !end_ta(p.ta_strongbox)) return -1;
      }
      lock.lock();
    }
    // If still in early boot the lock has never been released: a subsequent callback must
    // snapshot this generation. Destroy the retired (or rejected) profiles outside the lock.
    live.swap(next);
    publish_flags_locked();
    return static_cast<int>(live.size());
  }

 private:
  std::mutex& cfg_mu_;
  bool ended_ = false;
};

}  // namespace teesim