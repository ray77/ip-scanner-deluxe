//! Background tools: ping, traceroute (both native ICMP), port check.

use parking_lot::Mutex;
use platform::icmp::{echo_async, IcmpError, ReplyKind};
use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use ui::i18n::{t, tf};

#[derive(Default)]
pub struct ToolOutput {
    pub lines: Vec<String>,
    pub running: bool,
}

pub struct ToolSession {
    pub title: String,
    pub output: Arc<Mutex<ToolOutput>>,
    pub cancel: CancellationToken,
    pub id: u64,
}

impl ToolSession {
    fn new(ip: IpAddr, title: String, id: u64) -> Self {
        let _ = ip;
        ToolSession {
            title,
            output: Arc::new(Mutex::new(ToolOutput { lines: Vec::new(), running: true })),
            cancel: CancellationToken::new(),
            id,
        }
    }

    pub fn stop(&self) {
        self.cancel.cancel();
    }
}

type Repaint = Arc<dyn Fn() + Send + Sync>;

fn push(out: &Arc<Mutex<ToolOutput>>, line: String, repaint: &Repaint) {
    out.lock().lines.push(line);
    repaint();
}

fn ms(d: Duration) -> String {
    format!("{:.1}", d.as_secs_f64() * 1000.0)
}

pub fn start_ping(rt: &tokio::runtime::Handle, ip: IpAddr, continuous: bool, id: u64, repaint: Repaint) -> ToolSession {
    let s = ToolSession::new(ip, format!("{} {ip}", t("ping.title")), id);
    let out = s.output.clone();
    let cancel = s.cancel.clone();
    rt.spawn(async move {
        push(&out, format!("Ping {ip}"), &repaint);
        push(&out, String::new(), &repaint);
        let mut sent = 0u32;
        let mut rtts: Vec<f64> = Vec::new();
        let mut seq = 1u16;
        loop {
            if cancel.is_cancelled() || (!continuous && sent >= 4) {
                break;
            }
            sent += 1;
            match echo_async(ip, None, Duration::from_millis(2000), seq).await {
                Ok(r) if r.kind == ReplyKind::EchoReply => {
                    rtts.push(r.rtt.as_secs_f64() * 1000.0);
                    push(&out, tf("ping.reply", &[("ip", &r.from.to_string()), ("ms", &ms(r.rtt))]), &repaint);
                }
                Ok(r) => push(&out, tf("ping.unreach", &[("ip", &r.from.to_string())]), &repaint),
                Err(IcmpError::Timeout) => push(&out, t("ping.timeout").to_string(), &repaint),
                Err(e) => {
                    push(&out, tf("ping.no_icmp", &[("e", &e.to_string())]), &repaint);
                    break;
                }
            }
            seq = seq.wrapping_add(1);
            tokio::select! {
                _ = cancel.cancelled() => break,
                _ = tokio::time::sleep(Duration::from_millis(1000)) => {}
            }
        }
        let recv = rtts.len() as u32;
        let loss = ((sent - recv) * 100).checked_div(sent).unwrap_or(0);
        push(&out, String::new(), &repaint);
        push(&out, tf("ping.stats", &[("s", &sent.to_string()), ("r", &recv.to_string()), ("l", &loss.to_string())]), &repaint);
        if !rtts.is_empty() {
            let min = rtts.iter().copied().fold(f64::MAX, f64::min);
            let max = rtts.iter().copied().fold(0.0, f64::max);
            let avg = rtts.iter().sum::<f64>() / rtts.len() as f64;
            push(
                &out,
                tf("ping.rtt", &[("min", &format!("{min:.1}")), ("avg", &format!("{avg:.1}")), ("max", &format!("{max:.1}"))]),
                &repaint,
            );
        }
        out.lock().running = false;
        repaint();
    });
    s
}

pub fn start_traceroute(rt: &tokio::runtime::Handle, ip: IpAddr, id: u64, repaint: Repaint) -> ToolSession {
    let s = ToolSession::new(ip, format!("{} {ip}", t("trace.title")), id);
    let out = s.output.clone();
    let cancel = s.cancel.clone();
    rt.spawn(async move {
        push(&out, format!("Traceroute {ip}, max. 30 Hops"), &repaint);
        push(&out, String::new(), &repaint);
        for ttl in 1u8..=30 {
            if cancel.is_cancelled() {
                break;
            }
            let mut hop: Option<IpAddr> = None;
            let mut times = Vec::new();
            let mut done = false;
            for probe in 0..3u16 {
                match echo_async(ip, Some(ttl), Duration::from_millis(1500), u16::from(ttl) * 4 + probe).await {
                    Ok(r) => {
                        hop.get_or_insert(r.from);
                        times.push(format!("{} ms", ms(r.rtt)));
                        if matches!(r.kind, ReplyKind::EchoReply | ReplyKind::Unreachable(_)) {
                            done = true;
                        }
                    }
                    Err(IcmpError::Timeout) => times.push("*".into()),
                    Err(e) => {
                        push(&out, tf("ping.no_icmp", &[("e", &e.to_string())]), &repaint);
                        push(&out, t("trace.hint_perm").to_string(), &repaint);
                        out.lock().running = false;
                        repaint();
                        return;
                    }
                }
                if cancel.is_cancelled() {
                    break;
                }
            }
            let name = match hop {
                Some(h) => match protocols::dns::reverse_dns(h, Duration::from_millis(800)).await {
                    Some(n) => format!("{n} [{h}]"),
                    None => h.to_string(),
                },
                None => "*".into(),
            };
            push(&out, format!("{ttl:>3}   {:<28} {}", name, times.join("  ")), &repaint);
            if done {
                break;
            }
        }
        push(&out, String::new(), &repaint);
        push(&out, t("trace.done").to_string(), &repaint);
        out.lock().running = false;
        repaint();
    });
    s
}
