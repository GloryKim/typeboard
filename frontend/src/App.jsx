import { useState } from "react";

export default function App() {
  const [message, setMessage] = useState("");
  const [loading, setLoading] = useState(false);

  const fetchHello = async () => {
    setLoading(true);
    try {
      const res = await fetch("http://127.0.0.1:3001/api/hello");
      const data = await res.json();
      setMessage(data.message);
    } catch (e) {
      setMessage("백엔드에 연결할 수 없어요 😢");
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="app">
      <h1>typeboard</h1>
      <p className="subtitle">axum + React 심플 예제</p>
      <button onClick={fetchHello} disabled={loading}>
        {loading ? "불러오는 중..." : "백엔드에서 메시지 받기"}
      </button>
      {message && <p className="message">{message}</p>}
    </div>
  );
}
