import threading
import time
import unittest

from bench_lib.engines.watchdog import Watchdog


class Interrupts:
    def __init__(self):
        self.calls = 0

    def __call__(self):
        self.calls += 1


class WatchdogTest(unittest.TestCase):
    def watchdog(self, timeout_s):
        w = Watchdog(timeout_s)
        self.addCleanup(w.close)
        return w

    def test_fast_executions_are_never_interrupted(self):
        w = self.watchdog(0.2)
        hit = Interrupts()
        before = threading.active_count()
        for _ in range(1000):
            self.assertFalse(w.disarm(w.arm(hit)))
        self.assertLessEqual(threading.active_count(), before + 1)  # one thread, not 1000
        time.sleep(0.3)
        self.assertEqual(hit.calls, 0)

    def test_overrun_is_interrupted_once(self):
        w = self.watchdog(0.02)
        hit = Interrupts()
        token = w.arm(hit)
        time.sleep(0.15)
        self.assertEqual(hit.calls, 1)
        self.assertTrue(w.disarm(token))
        # The next execution starts clean and is not hit by the earlier deadline.
        other = Interrupts()
        self.assertFalse(w.disarm(w.arm(other)))
        self.assertEqual(other.calls, 0)

    def test_idle_thread_wakes_for_a_new_deadline(self):
        w = self.watchdog(0.02)
        w.start()
        time.sleep(0.05)  # idle: nothing armed
        hit = Interrupts()
        token = w.arm(hit)
        time.sleep(0.15)
        self.assertTrue(w.disarm(token))
        self.assertEqual(hit.calls, 1)

    def test_rearming_after_a_long_sleep(self):
        # The thread sleeps toward the first deadline; later ones are picked up then.
        w = self.watchdog(0.05)
        first = Interrupts()
        w.disarm(w.arm(first))
        second = Interrupts()
        token = w.arm(second)
        time.sleep(0.2)
        self.assertTrue(w.disarm(token))
        self.assertEqual((first.calls, second.calls), (0, 1))

    def test_failing_interrupt_and_close(self):
        w = Watchdog(0.01)

        def broken():
            raise RuntimeError("connection closed")

        token = w.arm(broken)
        time.sleep(0.08)
        self.assertTrue(w.disarm(token))  # the thread survived the exception
        w.close()
        w.close()
        self.assertTrue(w.closed)
        with self.assertRaises(RuntimeError):
            w.arm(broken)
        with self.assertRaises(ValueError):
            Watchdog(0)


if __name__ == "__main__":
    unittest.main()
