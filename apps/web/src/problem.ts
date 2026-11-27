export async function problemOf(res: Response | null, fallback: string) {
  const body = (await res?.json().catch(() => null)) as { message?: unknown; issues?: { issues: { message: string }[] }[] } | null | undefined;
  if (typeof body?.message === "string") return body.message;
  return body?.issues?.flatMap(({ issues }) => issues)[0]?.message ?? fallback;
}
