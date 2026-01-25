export enum ReauthErrorCode {
  /**
   * OAuth retry window expired. The OAuth flow must be restarted.
   */
  OAUTH_RETRY_EXPIRED = "OAUTH_RETRY_EXPIRED",
}

export type ReauthError = {
  code: ReauthErrorCode | string;
  message?: string;
};

export function requiresOAuthRestart(
  error: ReauthError | null | undefined,
): boolean {
  return error?.code === ReauthErrorCode.OAUTH_RETRY_EXPIRED;
}
