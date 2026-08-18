# ADR-0021: A dispatched agent outlives the client that started it

## Status

Accepted

## Context

Three workers in the gridded-statics run went inert mid-work at 56, 72, and roughly 60
minutes, costing an hour of silence each and a human investigation. The cause was traced
through the daemon supervisor log, the herdr server log, and both sources: a shutdown signal
reached the dispatch client process, and the client's own signal handler converted it into an
order to destroy the entire agent worker, closing every session inside it including the prompt
that was mid-flight. The daemon's reclamation sweep — the component the original brief blamed —
never ran once during the incident, and provably could not have: it waits for every mutating
command on the daemon to finish before acting, and a long prompt is a mutating command, so on a
machine with concurrent runs it starves permanently. That starvation is also the best
explanation for the roughly ninety orphaned agent processes censused on this machine.

Two directions were credible. Treat a dispatched agent as disposable — a client that receives a
shutdown signal is right to tear its agent down, since nothing will collect the result — which
is simple and leaves nothing running. Or treat the agent's work as durable, so a signal to the
client releases the agent rather than destroying it, and reclamation happens later on the basis
of idleness rather than on the basis of whose process died.

## Decision

A dispatched agent's in-flight work outlives the client process that started it. A shutdown
signal delivered to a dispatch client releases ownership of the running agent and exits; it
never issues an order that destroys a session with work in flight. The released agent keeps
working, completes, and is reclaimed afterwards by the residency sweep on the ordinary idleness
test. This binds only the destroy-on-signal path: an agent that is genuinely abandoned must
still be reclaimed, and reclamation must therefore be made to work on a busy daemon rather than
starving behind other runs' long prompts. Separately, and independently of who or what killed an
agent, a client waiting on a session that dies must learn within seconds rather than waiting out
a twenty-four-hour request timeout.

## Consequences

An hour of agent work is no longer forfeited to a stray signal, and every future variant of this
failure — whatever sends the signal next — costs a redispatch instead of a human investigation.
The cost is that a released agent keeps consuming subscription capacity on work that may never be
collected, and while it runs, a redispatched package can have two agents working the same problem
in different worktrees. That cost is bounded entirely by reclamation working, so the two changes
are one decision and must ship together; durability without a working sweep would convert every
abandoned agent into a permanent orphan, which is the state this machine is already in. The
decision also creates follow-up work the driver does not yet have: reattaching to a released
agent instead of redispatching it, which is where durability begins to pay rather than merely not
hurting. Finally, the origin of the shutdown signal remains unestablished; this decision
deliberately makes the run's correctness independent of that answer rather than waiting on it.
