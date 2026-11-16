import { deserializePageData } from "void/pages-server";

export const PAGE_HEADERS = { "X-VoidPages": "true", "X-VoidPages-Version": "2", Accept: "application/json" };

export function pageData<T>(body: string) {
  return deserializePageData(body) as T;
}
