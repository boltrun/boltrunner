"""Small deterministic helpers for authoring in-place character clips."""
import math


def smooth(value):
    value = max(0.0, min(1.0, value))
    return value * value * (3.0 - 2.0 * value)


def quat(axis, angle):
    value = [0.0, 0.0, 0.0, math.cos(angle / 2)]
    value[axis] = math.sin(angle / 2)
    return value


def multiply(a, b):
    x, y, z, w = a
    u, v, s, t = b
    return [w*u + x*t + y*s - z*v, w*v - x*s + y*t + z*u,
            w*s + x*v - y*u + z*t, w*t - x*u - y*v - z*s]


def rotation(x=0.0, y=0.0, z=0.0):
    return multiply(quat(2, z), multiply(quat(1, y), quat(0, x)))


def step(phase, stride, lift, stance=0.62):
    """A planted foot slides backwards at constant world travel speed."""
    phase %= 1.0
    if phase < stance:
        return stride * (1 - 2 * phase / stance), 0.0, 0.0
    progress = (phase - stance) / (1 - stance)
    height = math.sin(math.pi * progress) ** 2
    return -stride * math.cos(math.pi * progress), lift * height, height


def leg_angles(forward, down, upper, lower):
    """Two-link leg IK; returned angles face -Z and bend knees backwards."""
    distance = min(math.hypot(forward, down), (upper + lower) * 0.9998)
    distance = max(distance, abs(upper - lower) + 1e-5)
    knee = -math.acos(max(-1.0, min(1.0, (distance*distance - upper*upper - lower*lower) / (2*upper*lower))))
    hip = math.atan2(forward, down) + math.acos(max(-1.0, min(1.0, (upper*upper + distance*distance - lower*lower) / (2*upper*distance))))
    return hip, knee
