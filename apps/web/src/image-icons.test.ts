import { describe, expect, it } from "vite-plus/test";
import { imageNameProblem, MAX_IMAGE_NAME_LENGTH } from "./image-icons";

describe("imageNameProblem", () => {
  it("asks for a name, since screen readers, the CLI and Claude tell images apart only by it", () => {
    expect(imageNameProblem(null)).toBe("Enter a name");
  });

  it("refuses a name that would not fit the picker's tooltip and the CLI line", () => {
    expect(imageNameProblem("a".repeat(MAX_IMAGE_NAME_LENGTH))).toBeNull();
    expect(imageNameProblem("a".repeat(MAX_IMAGE_NAME_LENGTH + 1))).not.toBeNull();
  });

  it("refuses invisible characters, so two images cannot look like they have the same name", () => {
    expect(imageNameProblem("ミケ​")).not.toBeNull();
    expect(imageNameProblem("ミケ")).toBeNull();
  });
});
