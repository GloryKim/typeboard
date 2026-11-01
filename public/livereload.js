// Reloads the page when the server reports a change in docs/, and after the server
// restarts, which shows up as the socket closing and reconnecting.

const SCROLL_KEY = `livereload-scroll:${location.pathname}`;

const savedScroll = sessionStorage.getItem(SCROLL_KEY);
if (savedScroll !== null) {
  sessionStorage.removeItem(SCROLL_KEY);
  window.scrollTo(0, Number(savedScroll));
}

function reload() {
  sessionStorage.setItem(SCROLL_KEY, String(window.scrollY));
  location.reload();
}

function connect(reconnecting) {
  const protocol = location.protocol === "https:" ? "wss:" : "ws:";
  const socket = new WebSocket(`${protocol}//${location.host}/__livereload`);
  socket.addEventListener("open", () => {
    if (reconnecting) reload();
  });
  socket.addEventListener("message", reload);
  socket.addEventListener("close", () => setTimeout(() => connect(true), 1000));
}

connect(false);
