"""Continuous UI-thread liveness sampler.

/json/list is served by the browser process, so the time it takes to answer is a direct
read of how long the UI thread was unavailable. Sampling continuously for a window and
reporting the MAX is a much better instrument than a single one-shot probe, which only
sees a block if it happens to coincide with one.
"""
import json, sys, time, urllib.request

def ctl(h):
    urllib.request.urlopen("http://127.0.0.1:31401/__ctl?hang=%d" % (1 if h else 0), timeout=30).read()

def sample(seconds, label):
    end = time.time() + seconds
    worst, n, total = 0.0, 0, 0.0
    while time.time() < end:
        t = time.time()
        try:
            urllib.request.urlopen("http://127.0.0.1:9322/json/list", timeout=120).read()
            dt = time.time() - t
        except Exception:
            dt = time.time() - t
        worst = max(worst, dt); total += dt; n += 1
        time.sleep(0.1)
    print("%-22s samples=%3d  max=%7.3fs  mean=%6.3fs" % (label, n, worst, total / max(n, 1)))
    return worst

if __name__ == "__main__":
    secs = int(sys.argv[1]) if len(sys.argv) > 1 else 25
    ctl(False); time.sleep(1)
    a = sample(secs, "hang=0 (control)")
    ctl(True)
    b = sample(secs, "hang=1 (balance hung)")
    ctl(False); time.sleep(1)
    c = sample(secs, "hang=0 (control after)")
    print(json.dumps({"control_before_max": round(a,3), "hung_max": round(b,3),
                      "control_after_max": round(c,3)}))
