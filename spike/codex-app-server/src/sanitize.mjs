const SECRET_PATTERNS = [
  [/\bsk-[A-Za-z0-9_-]{8,}\b/g, "[REDACTED_API_KEY]"],
  [/\beyJ[A-Za-z0-9_.-]{16,}\b/g, "[REDACTED_TOKEN]"],
  [/[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}/gi, "[REDACTED_EMAIL]"],
];
export function sanitizeText(value) {
  let text = String(value ?? "");
  for (const [pattern, replacement] of SECRET_PATTERNS) text = text.replace(pattern, replacement);
  return text.slice(0, 2000);
}
function summarizeWindow(window, nowSeconds) {
  if (!window || typeof window !== "object") return null;
  const used = window.usedPercent;
  const reset = window.resetsAt;
  return {
    usedPercentPresent: Number.isFinite(used),
    usedPercentRangeValid: Number.isFinite(used) && used >= 0 && used <= 100,
    windowDurationMins: Number.isFinite(window.windowDurationMins) ? window.windowDurationMins : null,
    resetsAtPresent: Number.isFinite(reset),
    resetInFuture: Number.isFinite(reset) ? reset > nowSeconds : null,
  };
}
export function summarizeRateLimits(result, nowSeconds = Math.floor(Date.now() / 1000)) {
  const source = result?.rateLimitsByLimitId && typeof result.rateLimitsByLimitId === "object"
    ? Object.entries(result.rateLimitsByLimitId)
    : result?.rateLimits ? [[result.rateLimits.limitId ?? "legacy", result.rateLimits]] : [];
  return {
    snapshotPresent: Boolean(result?.rateLimits),
    multiBucketViewPresent: Boolean(result?.rateLimitsByLimitId),
    bucketCount: source.length,
    buckets: source.map(([key, bucket]) => ({
      key: sanitizeText(key), limitId: typeof bucket?.limitId === "string" ? sanitizeText(bucket.limitId) : null,
      limitNamePresent: typeof bucket?.limitName === "string", planTypePresent: typeof bucket?.planType === "string",
      primary: summarizeWindow(bucket?.primary, nowSeconds), secondary: summarizeWindow(bucket?.secondary, nowSeconds),
      reachedTypePresent: typeof bucket?.rateLimitReachedType === "string", creditsPresent: bucket?.credits != null,
    })),
    resetCreditsPresent: result?.rateLimitResetCredits != null, accountIdPresent: typeof result?.accountId === "string",
  };
}
export function summarizeAccount(result) {
  const account = result?.account;
  return {
    accountPresent: Boolean(account), accountType: typeof account?.type === "string" ? sanitizeText(account.type) : null,
    planTypePresent: typeof account?.planType === "string", emailPresent: typeof account?.email === "string",
    requiresOpenaiAuth: typeof result?.requiresOpenaiAuth === "boolean" ? result.requiresOpenaiAuth : null,
  };
}
