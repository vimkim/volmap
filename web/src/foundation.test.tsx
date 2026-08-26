import { renderToStaticMarkup } from "react-dom/server";
import { expect, test } from "vitest";

import { Foundation } from "./foundation";

test("the generated viewer foundation has an accessible readiness state", () => {
  expect(renderToStaticMarkup(<Foundation />)).toBe(
    '<p role="status">React viewer foundation ready.</p>',
  );
});
