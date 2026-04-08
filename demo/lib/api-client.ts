/**
 * HTTP client for Oxigit's Leptos server function endpoints.
 * Endpoint hashes match crates/oxigit-e2e/tests/harness/mod.rs.
 */

const ENDPOINTS = {
  register: "/api/register_user14969902946520757255",
  login: "/api/login_user9081721409912083587",
  createRepo: "/api/create_repo1862433574170527599",
} as const;

export class ApiClient {
  private baseUrl: string;
  private cookies: string[] = [];

  constructor(baseUrl: string) {
    this.baseUrl = baseUrl;
  }

  private async post(
    path: string,
    body: Record<string, string>,
  ): Promise<Response> {
    const res = await fetch(`${this.baseUrl}${path}`, {
      method: "POST",
      headers: {
        "Content-Type": "application/x-www-form-urlencoded",
        ...(this.cookies.length > 0
          ? { Cookie: this.cookies.join("; ") }
          : {}),
      },
      body: new URLSearchParams(body).toString(),
      redirect: "manual",
    });

    // Capture set-cookie headers
    const setCookies = res.headers.getSetCookie?.() ?? [];
    for (const sc of setCookies) {
      const name = sc.split("=")[0];
      // Replace existing cookie with same name
      this.cookies = this.cookies.filter((c) => !c.startsWith(`${name}=`));
      this.cookies.push(sc.split(";")[0]);
    }

    return res;
  }

  async healthCheck(): Promise<boolean> {
    try {
      const res = await fetch(this.baseUrl, { redirect: "manual" });
      return res.status < 500;
    } catch {
      return false;
    }
  }

  async register(
    username: string,
    email: string,
    password: string,
  ): Promise<void> {
    const res = await this.post(ENDPOINTS.register, {
      username,
      email,
      password,
    });
    if (res.status >= 400) {
      const text = await res.text();
      throw new Error(`Register failed (${res.status}): ${text}`);
    }
    console.log(`  Registered user: ${username}`);
  }

  async login(username: string, password: string): Promise<void> {
    const res = await this.post(ENDPOINTS.login, { username, password });
    if (res.status >= 400) {
      const text = await res.text();
      throw new Error(`Login failed (${res.status}): ${text}`);
    }
    console.log(`  Logged in as: ${username}`);
  }

  async createRepo(
    name: string,
    description: string,
    isPrivate: boolean,
  ): Promise<void> {
    const res = await this.post(ENDPOINTS.createRepo, {
      name,
      description,
      is_private: isPrivate ? "true" : "false",
    });
    if (res.status >= 400) {
      const text = await res.text();
      throw new Error(`Create repo failed (${res.status}): ${text}`);
    }
    console.log(`  Created repo: ${name}`);
  }
}
