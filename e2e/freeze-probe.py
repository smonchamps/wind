# Message-pump freeze probe (PLAN-GELS, decision D3).
#
# Measures the "window not responding" symptom the way Windows defines
# it: the main thread's message pump stops answering. Launches
# wind-desktop.exe (release) against a given base -- dummy account
# offline, same hooks as the E2E -- then probes the window every ~100 ms
# via SendMessageTimeout(WM_NULL) and logs every latency above the
# threshold. (Latency capped at 5 s by the call's timeout: a longer
# freeze counts as 5,000 ms for the probe that observes it.)
#
# Budget (HANDOVER §3): NO freeze > 150 ms after the window appears.
# Non-zero exit if the budget is exceeded.
#
#   python e2e/freeze-probe.py <base.db> [duration_s=40] [threshold_ms=150]
#
# NEVER during a gate or an e2e suite (2026-09-01): the e2e launcher
# sweeps every `wind-desktop` under target\ at each spec (`sweepZombies`,
# Stop-Process -Force) -- including the probe's release instance.
# Symptom: "the application stopped ... code 4294967295" (-1, the
# TerminateProcess code) after a few seconds, WebView2 stopped cleanly,
# no panic. This is not a product crash, it is the gate that killed the
# probe.
#
# The measurement base sits OUTSIDE the repository (OneDrive would
# distort the measurement) -- STANDARD §7.3. The founding field finding
# (2026-08-15, real base 251,062 envelopes, 17,761 NULL previews): 25.2 s
# of cumulative freezes over 40 s BEFORE the async switch of the
# commands, ~0 after.
#
# The instrument is verified like the rest (STANDARD §9): the
# application's output is DRAINED (a full pipe would block the child
# process and manufacture the very freeze being measured -- a lesson
# from launch.mjs) and printed back on failure; a dead process stops the
# probe by saying so (without this guard, a crash would print itself as
# false 0 ms freezes); the user32 calls carry their argtypes (a 64-bit
# HWND truncated to a C int would measure a phantom window).
import collections
import ctypes
import ctypes.wintypes as w
import json
import os
import subprocess
import sys
import threading
import time

if len(sys.argv) < 2:
    print("usage: python e2e/freeze-probe.py <base.db> [duration_s=40] [threshold_ms=150]")
    sys.exit(2)
db = os.path.abspath(sys.argv[1])
duration = float(sys.argv[2]) if len(sys.argv) > 2 else 40.0
threshold = float(sys.argv[3]) if len(sys.argv) > 3 else 150.0
if duration <= 0 or threshold <= 0:
    print(f"duration ({duration}) and threshold ({threshold}) must be positive -- an OK on 0 s would prove nothing")
    sys.exit(2)

user32 = ctypes.windll.user32
SMTO_BLOCK = 0x0001
WM_NULL = 0
# The argtypes/restype first: without them, ctypes truncates a 64-bit
# HWND into a 32-bit C int (OverflowError or silent sign extension).
user32.SendMessageTimeoutW.argtypes = [
    w.HWND, w.UINT, w.WPARAM, w.LPARAM, w.UINT, w.UINT,
    ctypes.POINTER(ctypes.c_size_t),
]
user32.SendMessageTimeoutW.restype = w.LPARAM
user32.IsWindowVisible.argtypes = [w.HWND]
user32.IsWindowVisible.restype = w.BOOL
user32.GetWindowThreadProcessId.argtypes = [w.HWND, ctypes.POINTER(w.DWORD)]
user32.GetWindowThreadProcessId.restype = w.DWORD

root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
exe = os.path.join(root, "target", "release", "wind-desktop.exe")
if not os.path.exists(exe):
    print(f"binary absent: {exe} -- build it first (cargo build -p wind-desktop --release)")
    sys.exit(2)

# Dedicated WebView2 profile NEXT TO the base: never the real
# application's, and outside the repository along with it.
profile = os.path.join(os.path.dirname(db), "webview2-probe")
os.makedirs(profile, exist_ok=True)

env = dict(os.environ)
env["WIND_DB_PATH"] = db
env["WIND_E2E_ACCOUNT"] = "sonde@exemple.fr"  # invalid token: offline guaranteed
env["WEBVIEW2_USER_DATA_FOLDER"] = profile
# OAuth purge: the LIST lives in isolation-oauth.json -- a SINGLE
# contract shared with the Node launchers (isolation.mjs): a provider
# added in one place covers every launcher. Without the purge, an OAuth
# route would open a real browser consent and suspend the probe.
with open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "isolation-oauth.json"), encoding="utf-8") as contract:
    for key in json.load(contract):
        env.pop(key, None)

t0 = time.perf_counter()
proc = subprocess.Popen([exe], env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)

# Drain the output continuously (Windows pipe: ~64 KB -- once full, the
# child blocks on write and the probe would measure its own artifact).
# Only the tail is kept, printed back on failure.
log = collections.deque(maxlen=200)


def drain():
    for line in proc.stdout:
        log.append(line.decode("utf-8", errors="replace").rstrip())


threading.Thread(target=drain, daemon=True).start()


def application_output():
    return "\n".join(["--- application output ---", *log, "--- end ---"]) if log else "(no output)"


def main_window(pid):
    """The process's visible top-level window."""
    views = []

    @ctypes.WINFUNCTYPE(w.BOOL, w.HWND, w.LPARAM)
    def cb(hwnd, _):
        if not user32.IsWindowVisible(hwnd):
            return True
        p = w.DWORD()
        user32.GetWindowThreadProcessId(hwnd, ctypes.byref(p))
        if p.value == pid:
            views.append(hwnd)
        return True

    user32.EnumWindows(cb, 0)
    return views[0] if views else None


def close(code):
    proc.kill()
    # Wait for the REAL exit: an immediate relaunch would pick up the
    # WebView2 profile while a process is still alive (a lesson from
    # closeApp).
    try:
        proc.wait(timeout=15)
    except subprocess.TimeoutExpired:
        print("the process does not die within 15 s after kill()")
    sys.exit(code)


hwnd = None
while hwnd is None and time.perf_counter() - t0 < 30:
    if proc.poll() is not None:
        print(f"FAILED the application stopped at startup (code {proc.returncode})")
        print(application_output())
        close(1)
    hwnd = main_window(proc.pid)
    if hwnd is None:
        time.sleep(0.05)
if hwnd is None:
    print("FAILED window never appeared within 30 s")
    print(application_output())
    close(1)

appeared = time.perf_counter() - t0
print(f"window appeared at t+{appeared * 1000:.0f} ms; probing {duration:.0f} s, threshold {threshold:.0f} ms")

freezes = []
end = t0 + appeared + duration
while time.perf_counter() < end:
    if proc.poll() is not None:
        # A dead process makes SendMessageTimeout fail immediately:
        # without this guard, a crash would print itself as false 0 ms
        # freezes.
        print(f"FAILED the application stopped at t+{time.perf_counter() - t0:.2f} s (code {proc.returncode}) -- this is not a freeze, it is a crash")
        print(application_output())
        close(1)
    before = time.perf_counter()
    res = ctypes.c_size_t()
    ok = user32.SendMessageTimeoutW(hwnd, WM_NULL, 0, 0, SMTO_BLOCK, 5000, ctypes.byref(res))
    latency = (time.perf_counter() - before) * 1000
    if latency > threshold or not ok:
        freezes.append(latency)
        print(
            f"FREEZE t+{time.perf_counter() - t0:.2f} s: pump blocked {latency:.0f} ms"
            f"{' (timeout)' if not ok else ''}",
            flush=True,
        )
    time.sleep(0.1)

total = sum(freezes) / 1000
if freezes:
    print(f"FAILED: {len(freezes)} freeze(s) > {threshold:.0f} ms, total {total:.2f} s over {duration:.0f} s")
    print(application_output())
    close(1)
print(f"OK: no freeze > {threshold:.0f} ms over {duration:.0f} s")
close(0)
