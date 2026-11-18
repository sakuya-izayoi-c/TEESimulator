// Exercise the production config handler and framing without starting its privileged server.
#include "../../common/control.cpp"

#include <cstdio>
#include <cstdlib>

namespace {
void Check(bool ok, const char* what) {
  if (!ok) {
    std::fprintf(stderr, "FAIL: %s\n", what);
    std::exit(1);
  }
}

int staged = 0;
bool reject_commit = false;
std::string deleted, salt;

void Push(const char* json, int want_epoch, bool want_ok, int want_applied, int want_failed) {
  tjson::Value msg, ack;
  Check(tjson::parse(json, std::strlen(json), msg), "parse config");
  uint64_t epoch;
  int total;
  int committed = ApplyConfig(msg, epoch, total);
  int fds[2];
  Check(pipe(fds) == 0, "pipe");
  Check(WriteFrame(fds[1], BuildAck(epoch, committed, total)), "send ack");
  close(fds[1]);
  std::string frame;
  Check(ReadFrame(fds[0], frame), "receive ack");
  close(fds[0]);
  Check(tjson::parse(frame.data(), frame.size(), ack), "parse ack");
  Check(ack.get("type") && ack.get("type")->as_string() == "ack", "ack type");
  Check(ack.get("epoch") && ack.get("epoch")->as_int(-1) == want_epoch, "ack epoch");
  Check(ack.get("ok") && ack.get("ok")->is_bool() && ack.get("ok")->as_bool() == want_ok,
        "ack ok must describe commit, not staging");
  Check(ack.get("profilesApplied") && ack.get("profilesApplied")->as_int(-1) == want_applied,
        "committed count");
  Check(ack.get("profilesFailed") && ack.get("profilesFailed")->as_int(-1) == want_failed,
        "failed count");
}
}  // namespace

// Only the C staging boundary and side effects are test doubles; no TAs or key material.
extern "C" void teesim_cfg_begin(const TsBootInfo*) { staged = 0; }
extern "C" bool teesim_cfg_add_profile(const TsProfile* p) {
  if (std::strcmp(p->id, "skip") == 0) return false;
  ++staged;
  return true;
}
extern "C" int teesim_cfg_commit(uint64_t, char* err, size_t len) {
  if (!reject_commit) return staged;
  std::snprintf(err, len, "early boot replay rejected");
  return -1;
}
extern "C" void teesim_cfg_set_rkp_deleted(const char* const* names, size_t n) {
  deleted = n ? names[0] : "";
}
void RedactSetSalt(const uint8_t* bytes, size_t size) {
  salt.assign(reinterpret_cast<const char*>(bytes), size);
}
extern "C" void teesim_log(int, const char*, const char*, ...) {}

int main() {
  Push(R"({"epoch":7,"profiles":[{"id":"keep"},{"id":"skip"}],"rkpDeleted":["old"],"redactSalt":"YQ=="})",
       7, true, 1, 1);
  Check(deleted == "old" && salt == "a", "successful push applies side effects");
  reject_commit = true;
  Push(R"({"epoch":8,"profiles":[{},{}],"rkpDeleted":["new"],"redactSalt":"Yg=="})",
       8, false, 0, 2);
  Check(deleted == "old" && salt == "a", "rejected push preserves side effects");
  reject_commit = false;
  Push(R"({"epoch":9,"profiles":[]})", 9, true, 0, 0);
  Check(deleted.empty() && salt == "a", "successful empty push retains protocol semantics");
  std::puts("PASS config ACK: partial success, rollback, empty replacement");
}