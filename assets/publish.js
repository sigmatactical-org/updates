(() => {
  const root = document.getElementById("updates-publish");
  if (!root) return;

  const identityBase = root.dataset.identityBase;
  const signInUrl = root.dataset.signInUrl;
  const publishApiUrl = root.dataset.publishApiUrl;
  const pending = document.getElementById("publish-auth-pending");
  const formWrap = document.getElementById("publish-form-wrap");
  const statusEl = document.getElementById("publish-status");
  const fileInput = document.getElementById("publish-file");
  const submitBtn = document.getElementById("publish-submit");

  function setStatus(text, isError) {
    if (!statusEl) return;
    statusEl.textContent = text;
    statusEl.classList.toggle("text-danger", !!isError);
    statusEl.classList.toggle("text-success", !isError && !!text);
  }

  async function authStatus() {
    const response = await fetch(new URL("auth/status", identityBase).toString(), {
      credentials: "include",
    });
    if (!response.ok) return null;
    return response.json();
  }

  async function csrfToken() {
    const response = await fetch(new URL("auth/csrftoken", identityBase).toString(), {
      method: "POST",
      credentials: "include",
    });
    if (!response.ok) throw new Error("CSRF token request failed");
    const data = await response.json();
    if (!data.token) throw new Error("Sign in required (empty CSRF token)");
    return data.token;
  }

  async function gate() {
    try {
      const status = await authStatus();
      if (!status || !status.authenticated) {
        if (pending) {
          pending.innerHTML =
            'Sign in with Identity to publish packages. <a class="link-light" href="' +
            signInUrl +
            '">Sign in</a>';
        }
        return;
      }
      if (pending) pending.classList.add("d-none");
      if (formWrap) formWrap.classList.remove("d-none");
    } catch {
      if (pending) {
        pending.textContent = "Unable to verify sign-in. Try again or use the Sign in link.";
        pending.classList.add("text-danger");
      }
    }
  }

  async function publish() {
    const file = fileInput && fileInput.files && fileInput.files[0];
    if (!file) {
      setStatus("Choose a .deb file first.", true);
      return;
    }
    if (!file.name.endsWith(".deb")) {
      setStatus("Filename must end with .deb", true);
      return;
    }
    setStatus("Publishing…", false);
    if (submitBtn) submitBtn.disabled = true;
    try {
      const token = await csrfToken();
      const response = await fetch(publishApiUrl, {
        method: "POST",
        credentials: "include",
        headers: {
          "X-CSRF-TOKEN": token,
          "X-Package-Filename": file.name,
          "Content-Type": "application/octet-stream",
        },
        body: file,
      });
      if (response.status === 401 || response.status === 403) {
        setStatus("Administrator access required (sigma-admin). Sign in and try again.", true);
        return;
      }
      if (!response.ok) {
        let detail = response.statusText;
        try {
          const err = await response.json();
          if (err.error) detail = err.error;
        } catch (_) {}
        setStatus("Publish failed: " + detail, true);
        return;
      }
      setStatus("Published " + file.name + ". Reloading…", false);
      window.setTimeout(() => window.location.reload(), 600);
    } catch (err) {
      setStatus(err && err.message ? err.message : "Publish failed.", true);
    } finally {
      if (submitBtn) submitBtn.disabled = false;
    }
  }

  if (submitBtn) submitBtn.addEventListener("click", (e) => {
    e.preventDefault();
    void publish();
  });

  void gate();
})();
