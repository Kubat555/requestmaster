<script lang="ts">
  import { strings } from "../lib/strings";
  import type { CookieDto } from "../lib/types";

  interface Props {
    /** Cookies the response set. */
    cookies: CookieDto[];
  }

  let { cookies }: Props = $props();

  /** When the cookie expires, as local date and time; a session cookie has no such moment. */
  function expires(cookie: CookieDto): string {
    if (cookie.expires_unix === null) return strings.response.session;
    return new Date(cookie.expires_unix * 1000).toLocaleString("ru-RU");
  }

  function flag(value: boolean): string {
    return value ? strings.response.yes : strings.response.no;
  }
</script>

{#if cookies.length === 0}
  <p class="note">{strings.response.noCookies}</p>
{:else}
  <table aria-label={strings.response.cookies}>
    <thead>
      <tr>
        <th scope="col">{strings.response.name}</th>
        <th scope="col">{strings.response.value}</th>
        <th scope="col">{strings.response.domain}</th>
        <th scope="col">{strings.response.path}</th>
        <th scope="col">{strings.response.expires}</th>
        <th scope="col">{strings.response.secure}</th>
        <th scope="col">{strings.response.httpOnly}</th>
      </tr>
    </thead>
    <tbody>
      {#each cookies as cookie, index (index)}
        <tr>
          <td>{cookie.name}</td>
          <td>{cookie.value}</td>
          <td>{cookie.domain}</td>
          <td>{cookie.path}</td>
          <td>{expires(cookie)}</td>
          <td>{flag(cookie.secure)}</td>
          <td>{flag(cookie.http_only)}</td>
        </tr>
      {/each}
    </tbody>
  </table>
{/if}

<style>
  .note {
    margin: 0;
    color: var(--text-muted);
  }

  table {
    width: 100%;
    border-collapse: collapse;
    font-family: var(--font-mono);
    overflow-wrap: anywhere;
  }

  th,
  td {
    padding: calc(var(--space) * 1.5) calc(var(--space) * 2);
    border: 1px solid var(--border);
    text-align: left;
    vertical-align: top;
  }

  th {
    color: var(--text-muted);
    font-family: var(--font-ui);
    font-weight: 600;
  }
</style>
