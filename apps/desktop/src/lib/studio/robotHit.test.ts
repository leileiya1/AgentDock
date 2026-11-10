import { expect, test } from "bun:test";
import { pickRobot } from "@/lib/studio/robotHit";

test("pickRobot returns the front-most robot under the point", () => {
  const boxes = [
    { id: "back", depth: 10, x: 0, y: 0, w: 20, h: 30 },
    { id: "front", depth: 20, x: 10, y: 5, w: 20, h: 30 },
  ];
  expect(pickRobot(boxes, 15, 10)).toBe("front");
  expect(pickRobot(boxes, 3, 3)).toBe("back");
  expect(pickRobot(boxes, 31, 20)).toBe("front"); // inside the 2px pad
  expect(pickRobot(boxes, 80, 80)).toBeNull();
});
