//go:build android

// Android has no /etc/resolv.conf. Route Go's SRV/TXT lookups through the active
// Android network's DNS servers, supplied by the foreground service.
package main

import (
    "context"
    "net"
    "os"
    "strings"
    "sync/atomic"
    "time"
)

func init() {
    servers := strings.Fields(os.Getenv("DOMINO_DNS_SERVERS"))
    if len(servers) == 0 { servers = []string{"1.1.1.1", "8.8.8.8"} }
    var next atomic.Uint64
    net.DefaultResolver.PreferGo = true
    net.DefaultResolver.Dial = func(ctx context.Context, network, _ string) (net.Conn, error) {
        server := servers[(next.Add(1)-1)%uint64(len(servers))]
        dialer := net.Dialer{Timeout: 5 * time.Second}
        return dialer.DialContext(ctx, network, net.JoinHostPort(server, "53"))
    }
}
