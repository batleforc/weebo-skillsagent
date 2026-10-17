// source: batlehub@20879e5 (ui/src/clientInit.ts), trimmed; axios client.
import { client } from "@/client/client.gen";

// Must be the first import in main.ts so baseURL is set before any request
// leaves a module. The axios client spells it `baseURL` (axios' own option);
// the fetch client spells it `baseUrl`. No trailing slash: the SDK appends
// "/api/v1/…", so one here doubles the slash in every request.
client.setConfig({ baseURL: import.meta.env.VITE_API_BASE_URL ?? "" });

// Bearer token: batlehub's useAuth calls this on login, refresh and logout.
export function setApiToken(token: string | undefined) {
  client.setConfig({ auth: token });
}
