"""A continuous boundary-distance guide for exact-raster geometry search.

Distances are an original four-neighbor signed Manhattan transform. Bilinear
sampling gives continuous proposal gradients. This is a surrogate only: every
accepted integer design-coordinate change is scored by the original TTF engine.
Quadratic basis functions follow the OpenType TrueType outline representation:
https://learn.microsoft.com/en-us/typography/opentype/spec/ttch01
"""

from collections import deque
from copy import deepcopy
import math

import reconstruct_geometry as geometry
from reconstruct_font import ppem


class DistanceField:
    def __init__(self, pixels):
        if not pixels:
            raise ValueError("boundary guide requires nonblank silhouettes")
        self.left = min(x for x, _ in pixels) - 8
        self.top = min(y for _, y in pixels) - 8
        self.width = max(x for x, _ in pixels) - self.left + 9
        self.height = max(y for _, y in pixels) - self.top + 9
        n = self.width * self.height
        if n > 4_000_000:
            raise ValueError("distance-field budget exceeded")
        inside = bytearray(n)
        for x, y in pixels:
            inside[(y - self.top) * self.width + x - self.left] = 1
        distances = [-1] * n
        todo = deque()
        for y in range(1, self.height - 1):
            for x in range(1, self.width - 1):
                i = y * self.width + x
                if any(
                    inside[j] != inside[i]
                    for j in (i - 1, i + 1, i - self.width, i + self.width)
                ):
                    distances[i] = 0
                    todo.append(i)
        while todo:
            i = todo.popleft()
            x, y = i % self.width, i // self.width
            for dx, dy in ((-1, 0), (1, 0), (0, -1), (0, 1)):
                nx, ny = x + dx, y + dy
                if 0 <= nx < self.width and 0 <= ny < self.height:
                    j = ny * self.width + nx
                    if distances[j] < 0:
                        distances[j] = distances[i] + 1
                        todo.append(j)
        self.values = [
            (d + 0.5) * (-1 if ink else 1) for d, ink in zip(distances, inside)
        ]

    def sample(self, x, y):
        # Pixels occupy unit cells; samples are located at cell centers.
        x, y = x - self.left - 0.5, y - self.top - 0.5
        if not 0 <= x < self.width - 1 or not 0 <= y < self.height - 1:
            return 16.0, 0.0, 0.0
        ix, iy = math.floor(x), math.floor(y)
        u, v = x - ix, y - iy
        i = iy * self.width + ix
        a, b, c, d = (
            self.values[j] for j in (i, i + 1, i + self.width, i + self.width + 1)
        )
        value = (1 - v) * ((1 - u) * a + u * b) + v * ((1 - u) * c + u * d)
        dx = (1 - v) * (b - a) + v * (d - c)
        dy = (1 - u) * (c - a) + u * (d - b)
        return value, dx, dy


def samples(shape):
    """Sparse linear weights for evaluating curve samples and their derivatives."""
    result, offset = [], 0
    for contour in shape:
        i, n = 0, len(contour)
        while i < n:
            a, b = i, (i + 1) % n
            if contour[b][2]:
                for t in (0.0, 0.25, 0.5, 0.75):
                    result.append([(offset + a, 1 - t), (offset + b, t)])
                i += 1
            else:
                c = (i + 2) % n
                for t in (0.0, 0.125, 0.25, 0.375, 0.5, 0.625, 0.75, 0.875):
                    result.append(
                        [
                            (offset + a, (1 - t) ** 2),
                            (offset + b, 2 * t * (1 - t)),
                            (offset + c, t * t),
                        ]
                    )
                i += 2
        offset += n
    return result


def freedoms(shape, graph):
    """Tie straight edges, exact tangents and semantic feature coordinates."""
    points = [p for contour in shape for p in contour]
    result = []
    for axis in (0, 1):
        parents = list(range(len(points)))

        def root(i):
            while parents[i] != i:
                parents[i] = parents[parents[i]]
                i = parents[i]
            return i

        def join(a, b):
            parents[root(b)] = root(a)

        offset = 0
        for contour in shape:
            for i, p in enumerate(contour):
                j = (i + 1) % len(contour)
                if p[axis] == contour[j][axis]:
                    join(offset + i, offset + j)
            offset += len(contour)
        for group in graph[axis]["groups"]:
            for i in group["points"][1:]:
                join(group["points"][0], i)
        components = {}
        for i in range(len(points)):
            components.setdefault(root(i), []).append(i)
        result.extend((axis, indices) for indices in components.values())
    return result


class Guide:
    def __init__(self, references, codepoint):
        self.fields = [
            (ppem(q[0]) / 2048, DistanceField(p))
            for q, p in references.items()
            if q[2] == codepoint and q[0] == q[1] and q[3] == 0
        ]
        if not self.fields:
            raise ValueError("geometry guide lacks upright square captures")

    def gradient(self, shape):
        points = [p for contour in shape for p in contour]
        gradient = [[0.0, 0.0] for _ in points]
        basis = samples(shape)
        for weights in basis:
            x = sum(points[i][0] * w for i, w in weights)
            y = sum(points[i][1] * w for i, w in weights)
            for scale, field in self.fields:
                distance, dx, dy = field.sample(x * scale, -y * scale)
                for i, weight in weights:
                    gradient[i][0] += 2 * distance * dx * scale * weight / len(basis)
                    gradient[i][1] -= 2 * distance * dy * scale * weight / len(basis)
        return gradient

    def proposals(self, shape, graph, step, limit):
        gradient = self.gradient(shape)
        options = []
        for axis, indices in freedoms(shape, graph):
            derivative = sum(gradient[i][axis] for i in indices)
            for delta in (-step, step):
                options.append((delta * derivative, axis, indices, delta))
        # Always include both signs for some feature groups: a joint hint/outline
        # improvement need not improve the continuous large-size surrogate.
        ranked = sorted(options, key=lambda row: (row[0], row[1], row[2], row[3]))
        chosen = ranked[:limit]
        for axis, feature in enumerate(graph):
            for group in feature["groups"]:
                for option in options:
                    if (
                        option[1] == axis
                        and group["points"][0] in option[2]
                        and option not in chosen
                    ):
                        chosen.append(option)
        return [(axis, indices, delta) for _, axis, indices, delta in chosen]


def moved(shape, baseline, axis, indices, delta, bound=12):
    candidate = [[list(p) for p in contour] for contour in shape]
    points = [p for c in candidate for p in c]
    original = [p for c in baseline for p in c]
    for i in indices:
        points[i][axis] += delta
        if abs(points[i][axis] - original[i][axis]) > bound:
            return None
    if not geometry.topology_matches(baseline, candidate):
        return None
    return candidate
