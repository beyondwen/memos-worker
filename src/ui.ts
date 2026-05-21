export function appHtml(): string {
  return `<!doctype html>
<html lang="zh-CN">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Memos Worker</title>
  <style>
    :root { color-scheme: light; font-family: ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif; background: #f6f7f8; color: #1d252c; }
    * { box-sizing: border-box; }
    body { margin: 0; min-height: 100vh; }
    .shell { display: grid; grid-template-columns: 280px 1fr; min-height: 100vh; }
    aside { background: #ffffff; border-right: 1px solid #dde3e8; padding: 24px; position: sticky; top: 0; height: 100vh; }
    main { padding: 24px; max-width: 980px; width: 100%; }
    h1 { font-size: 22px; margin: 0 0 8px; }
    h2 { font-size: 16px; margin: 24px 0 10px; }
    label { display: block; font-size: 13px; font-weight: 650; margin: 12px 0 6px; color: #42515d; }
    input, textarea, select { width: 100%; border: 1px solid #c9d2da; border-radius: 6px; padding: 10px 11px; font: inherit; background: #fff; }
    textarea { min-height: 140px; resize: vertical; }
    button { border: 1px solid #1f6feb; background: #1f6feb; color: #fff; border-radius: 6px; padding: 9px 13px; font: inherit; font-weight: 650; cursor: pointer; }
    button.secondary { background: #fff; color: #1f2937; border-color: #c9d2da; }
    button.danger { background: #c93535; border-color: #c93535; }
    .row { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; }
    .muted { color: #687782; font-size: 13px; }
    .panel { background: #fff; border: 1px solid #dde3e8; border-radius: 8px; padding: 16px; margin-bottom: 16px; }
    .memo { background: #fff; border: 1px solid #dde3e8; border-radius: 8px; padding: 14px; margin-bottom: 12px; }
    .memo header { display: flex; justify-content: space-between; gap: 10px; color: #687782; font-size: 12px; margin-bottom: 8px; }
    .memo pre { white-space: pre-wrap; font-family: inherit; margin: 0 0 12px; line-height: 1.55; }
    .attachments { display: flex; flex-wrap: wrap; gap: 8px; margin: 0 0 12px; }
    .attachments a { color: #1f6feb; font-size: 13px; text-decoration: none; border: 1px solid #c9d2da; border-radius: 999px; padding: 4px 9px; }
    .editBox { display: grid; gap: 8px; margin: 0 0 12px; }
    .hidden { display: none; }
    .error { color: #b42318; font-size: 13px; min-height: 18px; }
    @media (max-width: 760px) { .shell { grid-template-columns: 1fr; } aside { position: static; height: auto; } main { padding: 16px; } }
  </style>
</head>
<body>
  <div class="shell">
    <aside>
      <h1>Memos Worker</h1>
      <div id="user" class="muted"></div>
      <div id="authPanel">
        <h2 id="authTitle">登录</h2>
        <label>用户名</label>
        <input id="username" autocomplete="username">
        <label>密码</label>
        <input id="password" type="password" autocomplete="current-password">
        <label class="setupOnly hidden">昵称</label>
        <input id="nickname" class="setupOnly hidden">
        <p id="authError" class="error"></p>
        <button id="authButton">登录</button>
      </div>
      <div id="sessionPanel" class="hidden">
        <h2>操作</h2>
        <div class="row">
          <button id="refreshButton" class="secondary">刷新</button>
          <button id="logoutButton" class="secondary">登出</button>
        </div>
        <h2>修改密码</h2>
        <label>当前密码</label>
        <input id="currentPassword" type="password" autocomplete="current-password">
        <label>新密码</label>
        <input id="newPassword" type="password" autocomplete="new-password">
        <p id="passwordError" class="error"></p>
        <button id="changePasswordButton" class="secondary">更新密码</button>
      </div>
    </aside>
    <main>
      <section id="composer" class="panel hidden">
        <h2>新 memo</h2>
        <textarea id="content" placeholder="记录一点什么，支持 #标签"></textarea>
        <label>可见性</label>
        <select id="visibility">
          <option value="PRIVATE">私有</option>
          <option value="PROTECTED">登录可见</option>
          <option value="PUBLIC">公开</option>
        </select>
        <label>附件</label>
        <input id="files" type="file" multiple>
        <p id="memoError" class="error"></p>
        <button id="createButton">发布</button>
      </section>
      <section>
        <div class="row" style="justify-content: space-between;">
          <h2>列表</h2>
          <input id="tagFilter" placeholder="按标签筛选" style="max-width: 180px;">
        </div>
        <div id="memos"></div>
        <button id="moreButton" class="secondary hidden">加载更多</button>
      </section>
    </main>
  </div>
  <script>
    let accessToken = localStorage.getItem("memos_access") || "";
    let setupRequired = false;
    let nextPageToken = "";

    const $ = (id) => document.getElementById(id);
    const api = async (path, options = {}) => {
      const headers = new Headers(options.headers || {});
      if (!(options.body instanceof FormData)) headers.set("Content-Type", "application/json");
      if (accessToken) headers.set("Authorization", "Bearer " + accessToken);
      const response = await fetch(path, { ...options, headers });
      const data = await response.json().catch(() => ({}));
      if (!response.ok) throw new Error(data.error || "请求失败");
      return data;
    };

    async function boot() {
      const instance = await api("/api/v1/instance");
      setupRequired = instance.setupRequired;
      $("authTitle").textContent = setupRequired ? "创建管理员" : "登录";
      $("authButton").textContent = setupRequired ? "创建并登录" : "登录";
      document.querySelectorAll(".setupOnly").forEach((node) => node.classList.toggle("hidden", !setupRequired));
      if (accessToken) await loadUser().catch(() => { accessToken = ""; localStorage.removeItem("memos_access"); });
    }

    async function loadUser() {
      const data = await api("/api/v1/auth/user");
      $("user").textContent = data.user.nickname || data.user.username;
      $("authPanel").classList.add("hidden");
      $("sessionPanel").classList.remove("hidden");
      $("composer").classList.remove("hidden");
      await loadMemos(true);
      const events = new EventSource("/api/v1/sse?access_token=" + encodeURIComponent(accessToken));
      events.addEventListener("memo.created", () => loadMemos(true));
      events.addEventListener("memo.updated", () => loadMemos(true));
      events.addEventListener("memo.deleted", () => loadMemos(true));
    }

    async function loadMemos(reset = false) {
      if (reset) nextPageToken = "";
      const params = new URLSearchParams({ page_size: "30" });
      if (nextPageToken) params.set("page_token", nextPageToken);
      if ($("tagFilter").value.trim()) params.set("tag", $("tagFilter").value.trim());
      const data = await api("/api/v1/memos?" + params);
      nextPageToken = data.nextPageToken || "";
      $("moreButton").classList.toggle("hidden", !nextPageToken);
      if (reset) $("memos").innerHTML = "";
      for (const memo of data.memos) renderMemo(memo);
    }

    function renderMemo(memo) {
      const article = document.createElement("article");
      article.className = "memo";
      article.innerHTML = '<header><span></span><span></span></header><pre></pre><div class="attachments"></div><div class="reactions"></div><div class="row"><button class="secondary edit">编辑</button><button class="secondary archive">归档</button><button class="secondary react">Reaction</button><button class="secondary comment">评论</button><button class="secondary share">分享</button></div><div class="comments hidden"></div>';
      article.querySelector("header span:first-child").textContent = memo.creator.username + " · " + new Date(memo.createdTs * 1000).toLocaleString();
      article.querySelector("header span:last-child").textContent = memo.visibility;
      article.querySelector("pre").textContent = memo.content;

      const attachments = article.querySelector(".attachments");
      for (const attachment of memo.attachments || []) {
        const link = document.createElement("a");
        link.href = attachment.url;
        link.textContent = attachment.filename;
        link.target = "_blank";
        attachments.appendChild(link);
      }
      attachments.classList.toggle("hidden", !attachments.children.length);

      loadReactions(article, memo.uid);
      article.querySelector(".edit").onclick = () => openEditor(article, memo);
      article.querySelector(".archive").onclick = async () => {
        await api("/api/v1/memos/" + encodeURIComponent(memo.uid), { method: "DELETE" });
        await loadMemos(true);
      };
      article.querySelector(".react").onclick = () => addReaction(article, memo.uid);
      article.querySelector(".comment").onclick = () => toggleComments(article, memo.uid);
      article.querySelector(".share").onclick = () => createShareLink(memo.uid);
      $("memos").appendChild(article);
    }

    async function loadReactions(article, memoUid) {
      try {
        const data = await api("/api/v1/memos/" + encodeURIComponent(memoUid) + "/reactions");
        const container = article.querySelector(".reactions");
        container.innerHTML = "";
        const grouped = {};
        for (const r of data.reactions || []) {
          if (!grouped[r.reactionType]) grouped[r.reactionType] = [];
          grouped[r.reactionType].push(r);
        }
        for (const [type, list] of Object.entries(grouped)) {
          const span = document.createElement("span");
          span.className = "reaction-badge";
          span.textContent = type + " " + list.length;
          span.style.cssText = "border:1px solid #c9d2da;border-radius:999px;padding:2px 8px;font-size:13px;margin-right:4px;cursor:pointer;";
          container.appendChild(span);
        }
        container.classList.toggle("hidden", !container.children.length);
      } catch {}
    }

    async function addReaction(article, memoUid) {
      const type = prompt("输入 reaction (例如: 👍, ❤️, 👀):");
      if (!type) return;
      try {
        await api("/api/v1/memos/" + encodeURIComponent(memoUid) + "/reactions", {
          method: "POST",
          body: JSON.stringify({ reactionType: type })
        });
        await loadReactions(article, memoUid);
      } catch (err) {
        alert(err.message);
      }
    }

    async function toggleComments(article, memoUid) {
      const container = article.querySelector(".comments");
      if (!container.classList.contains("hidden")) { container.classList.add("hidden"); return; }
      container.classList.remove("hidden");
      container.innerHTML = "<p class='muted'>加载中...</p>";
      try {
        const data = await api("/api/v1/memos/" + encodeURIComponent(memoUid) + "/comments");
        container.innerHTML = "";
        for (const c of data.memos || []) {
          const div = document.createElement("div");
          div.style.cssText = "border-top:1px solid #eee;padding:8px 0;font-size:13px;";
          div.innerHTML = "<strong></strong> <span class='muted'></span><pre style='margin:4px 0 0'></pre>";
          div.querySelector("strong").textContent = c.creator.username;
          div.querySelector(".muted").textContent = new Date(c.createdTs * 1000).toLocaleString();
          div.querySelector("pre").textContent = c.content;
          container.appendChild(div);
        }
        const form = document.createElement("div");
        form.className = "row";
        form.style.marginTop = "8px";
        form.innerHTML = '<input placeholder="写评论..." style="flex:1"><button>发送</button>';
        form.querySelector("button").onclick = async () => {
          const input = form.querySelector("input");
          if (!input.value.trim()) return;
          try {
            await api("/api/v1/memos/" + encodeURIComponent(memoUid) + "/comments", {
              method: "POST",
              body: JSON.stringify({ content: input.value })
            });
            input.value = "";
            await toggleComments(article, memoUid);
            await toggleComments(article, memoUid);
          } catch (err) { alert(err.message); }
        };
        container.appendChild(form);
      } catch (err) {
        container.innerHTML = "<p class='error'>" + err.message + "</p>";
      }
    }

    async function createShareLink(memoUid) {
      try {
        const data = await api("/api/v1/memos/" + encodeURIComponent(memoUid) + "/shares", {
          method: "POST",
          body: JSON.stringify({})
        });
        const url = location.origin + data.share.url;
        await navigator.clipboard.writeText(url).catch(() => {});
        alert("分享链接已创建并复制到剪贴板:\\n" + url);
      } catch (err) { alert(err.message); }
    }

    function openEditor(article, memo) {
      const old = article.querySelector(".editBox");
      if (old) old.remove();

      const box = document.createElement("div");
      box.className = "editBox";
      box.innerHTML = '<textarea></textarea><select><option value="PRIVATE">私有</option><option value="PROTECTED">登录可见</option><option value="PUBLIC">公开</option></select><input type="file" multiple><div class="row"><button>保存</button><button class="secondary cancel">取消</button></div><p class="error"></p>';
      box.querySelector("textarea").value = memo.content;
      box.querySelector("select").value = memo.visibility;
      article.querySelector("pre").after(box);

      box.querySelector(".cancel").onclick = () => box.remove();
      box.querySelector("button").onclick = async () => {
        const error = box.querySelector(".error");
        error.textContent = "";
        try {
          await api("/api/v1/memos/" + encodeURIComponent(memo.uid), {
            method: "PATCH",
            body: JSON.stringify({
              content: box.querySelector("textarea").value,
              visibility: box.querySelector("select").value
            })
          });
          await uploadFiles(memo.uid, box.querySelector("input").files);
          await loadMemos(true);
        } catch (err) {
          error.textContent = err.message;
        }
      };
    }

    async function uploadFiles(memoUid, files) {
      for (const file of Array.from(files || [])) {
        const form = new FormData();
        form.set("file", file);
        form.set("memoUid", memoUid);
        await api("/api/v1/attachments", { method: "POST", body: form });
      }
    }

    $("authButton").onclick = async () => {
      $("authError").textContent = "";
      try {
        const path = setupRequired ? "/api/v1/setup" : "/api/v1/auth/signin";
        const data = await api(path, {
          method: "POST",
          body: JSON.stringify({
            username: $("username").value,
            password: $("password").value,
            nickname: $("nickname").value
          })
        });
        accessToken = data.accessToken;
        localStorage.setItem("memos_access", accessToken);
        await loadUser();
      } catch (error) {
        $("authError").textContent = error.message;
      }
    };

    $("createButton").onclick = async () => {
      $("memoError").textContent = "";
      try {
        const data = await api("/api/v1/memos", {
          method: "POST",
          body: JSON.stringify({ content: $("content").value, visibility: $("visibility").value })
        });
        await uploadFiles(data.memo.uid, $("files").files);
        $("content").value = "";
        $("files").value = "";
        await loadMemos(true);
      } catch (error) {
        $("memoError").textContent = error.message;
      }
    };

    $("changePasswordButton").onclick = async () => {
      $("passwordError").textContent = "";
      try {
        await api("/api/v1/auth/change-password", {
          method: "POST",
          body: JSON.stringify({
            currentPassword: $("currentPassword").value,
            newPassword: $("newPassword").value
          })
        });
        localStorage.removeItem("memos_access");
        alert("密码已更新，请重新登录");
        location.reload();
      } catch (error) {
        $("passwordError").textContent = error.message;
      }
    };

    $("refreshButton").onclick = () => loadMemos(true);
    $("moreButton").onclick = () => loadMemos(false);
    $("tagFilter").onchange = () => loadMemos(true);
    $("logoutButton").onclick = async () => {
      await api("/api/v1/auth/signout", { method: "POST", body: "{}" }).catch(() => undefined);
      localStorage.removeItem("memos_access");
      location.reload();
    };

    boot().catch((error) => { $("authError").textContent = error.message; });
  </script>
</body>
</html>`;
}
