/**
 * GLITCH WARS - Notification Feed Component
 * Real-time event notifications
 */

import { useGameStore } from '../store/gameStore';

export function NotificationFeed() {
  const { notifications, removeNotification } = useGameStore();

  const typeStyles = {
    info: 'border-terminal-blue bg-terminal-blue/10 text-terminal-blue',
    success: 'border-terminal-green bg-terminal-green/10 text-terminal-green',
    warning: 'border-terminal-yellow bg-terminal-yellow/10 text-terminal-yellow',
    danger: 'border-terminal-red bg-terminal-red/10 text-terminal-red',
  };

  const typeIcons = {
    info: 'ℹ',
    success: '✓',
    warning: '⚠',
    danger: '✕',
  };

  if (notifications.length === 0) return null;

  return (
    <div className="fixed top-16 right-4 z-50 w-80 space-y-2">
      {notifications.map((notif) => (
        <div
          key={notif.id}
          className={`
            p-3 rounded-lg border backdrop-blur-sm
            animate-[slideIn_0.3s_ease-out]
            ${typeStyles[notif.type]}
          `}
          style={{
            animation: 'slideIn 0.3s ease-out',
          }}
        >
          <div className="flex items-center gap-2">
            <span className="text-lg">{typeIcons[notif.type]}</span>
            <span className="flex-1 text-sm font-mono uppercase tracking-wider">
              {notif.message}
            </span>
            <button
              onClick={() => removeNotification(notif.id)}
              className="opacity-50 hover:opacity-100 transition-opacity"
            >
              ✕
            </button>
          </div>
        </div>
      ))}
      
      <style>{`
        @keyframes slideIn {
          from {
            opacity: 0;
            transform: translateX(20px);
          }
          to {
            opacity: 1;
            transform: translateX(0);
          }
        }
      `}</style>
    </div>
  );
}

export default NotificationFeed;
