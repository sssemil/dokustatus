"use client";

import { createContext, useContext, useState, useCallback, type ReactNode } from "react";
import { createReauthClient, type TokenResponse } from "@reauth/sdk";

const DOMAIN = process.env.NEXT_PUBLIC_DOMAIN || "demo.test";

type TokenContextType = {
  token: string | null;
  loading: boolean;
  fetchToken: () => Promise<string | null>;
  clearToken: () => void;
};

const TokenContext = createContext<TokenContextType>({
  token: null,
  loading: true,
  fetchToken: async () => null,
  clearToken: () => {},
});

export function TokenProvider({ children }: { children: ReactNode }) {
  const [token, setToken] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  const fetchToken = useCallback(async () => {
    const client = createReauthClient({ domain: DOMAIN });
    try {
      const tokenResponse = await client.getToken();
      if (tokenResponse) {
        setToken(tokenResponse.accessToken);
        return tokenResponse.accessToken;
      }
      setToken(null);
      return null;
    } catch {
      setToken(null);
      return null;
    } finally {
      setLoading(false);
    }
  }, []);

  const clearToken = useCallback(() => {
    setToken(null);
  }, []);

  return (
    <TokenContext.Provider value={{ token, loading, fetchToken, clearToken }}>
      {children}
    </TokenContext.Provider>
  );
}

export function useToken() {
  return useContext(TokenContext);
}
