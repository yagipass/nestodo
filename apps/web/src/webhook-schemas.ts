import { z } from "zod";
import { MAX_TITLE_LENGTH } from "./todos";

const MAX_URL_LENGTH = 2000;
const MAX_TOKEN_LENGTH = 4000;
const MAX_HEADERS = 20;
const MAX_HEADER_NAME_LENGTH = 200;
const MAX_HEADER_VALUE_LENGTH = 2000;

export const saveWebhookBody = z.object({
  url: z.string().trim().max(MAX_URL_LENGTH, `The URL must be ${MAX_URL_LENGTH} characters or fewer`),
  urlSecret: z.boolean(),
  token: z
    .string()
    .trim()
    .max(MAX_TOKEN_LENGTH, `The token must be ${MAX_TOKEN_LENGTH} characters or fewer`)
    .regex(/^[\x21-\x7e]*$/, "The token can only contain visible ASCII characters")
    .nullable(),
  headers: z
    .array(
      z.object({
        name: z.string().max(MAX_HEADER_NAME_LENGTH, `A header name must be ${MAX_HEADER_NAME_LENGTH} characters or fewer`),
        value: z.string().max(MAX_HEADER_VALUE_LENGTH, `A header value must be ${MAX_HEADER_VALUE_LENGTH} characters or fewer`),
        secret: z.boolean(),
      }),
    )
    .max(MAX_HEADERS, `List ${MAX_HEADERS} headers or fewer`),
  project: z.string().trim().min(1, "Enter the title of a top-level todo").max(MAX_TITLE_LENGTH, `The title must be ${MAX_TITLE_LENGTH} characters or fewer`),
});
