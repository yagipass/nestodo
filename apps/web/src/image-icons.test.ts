import { describe, expect, it } from "vite-plus/test";
import { imageNameProblem, MAX_IMAGE_NAME_LENGTH, nameFromFile } from "./image-icons";

describe("nameFromFile", () => {
  it("drops the extension, so the name field starts with what the member called the picture", () => {
    expect(nameFromFile("mike.jpg")).toBe("mike");
    expect(nameFromFile("IMG 2041.HEIC")).toBe("IMG 2041");
    expect(nameFromFile("school.logo.png")).toBe("school.logo");
  });

  it("leaves the field empty for a file with no name, so the member is asked to type one", () => {
    expect(nameFromFile(".png")).toBe("");
    expect(nameFromFile("   .jpg")).toBe("");
  });

  it("cuts a long file name to what the server accepts without splitting an emoji, so Add and use is not refused", () => {
    const name = nameFromFile(`${"a".repeat(MAX_IMAGE_NAME_LENGTH - 1)}🐈.png`);
    expect(name).toBe("a".repeat(MAX_IMAGE_NAME_LENGTH - 1));
    expect(imageNameProblem(name)).toBeNull();
    expect(nameFromFile(`${"a".repeat(MAX_IMAGE_NAME_LENGTH + 5)}.png`)).toHaveLength(MAX_IMAGE_NAME_LENGTH);
  });
});

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
