//! Pure authoritative rules. Persist `Game`; send only `Game::view` to clients.
//! The host supplies authenticated seats, serialized commands and monotonic milliseconds.
use serde::{Deserialize, Serialize};

pub const RULES_VERSION: &str = "friends-108-number-batch-zero-five-v2";
pub const TURN_MS: u64 = 30_000;
pub const UNO_MS: u64 = 2_000;
pub const CHALLENGE_MS: u64 = 8_000;
pub const SWAP_MS: u64 = 15_000;
pub const FIVE_READY_MS: u64 = 1_200;
pub const FIVE_MS: u64 = 6_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Color {
    Red,
    Yellow,
    Green,
    Blue,
}
impl Color {
    pub const ALL: [Self; 4] = [Self::Red, Self::Yellow, Self::Green, Self::Blue];
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Value {
    Number(u8),
    Skip,
    Reverse,
    DrawTwo,
    Wild,
    WildFour,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Card {
    pub id: u8,
    pub color: Option<Color>,
    pub value: Value,
}
pub fn deck() -> Vec<Card> {
    let mut cards = Vec::new();
    for color in Color::ALL {
        for n in 0..=9 {
            for _ in 0..if n == 0 { 1 } else { 2 } {
                cards.push((Some(color), Value::Number(n)));
            }
        }
        for value in [Value::Skip, Value::Reverse, Value::DrawTwo] {
            for _ in 0..2 {
                cards.push((Some(color), value));
            }
        }
    }
    for value in [Value::Wild, Value::WildFour] {
        for _ in 0..4 {
            cards.push((None, value));
        }
    }
    cards
        .into_iter()
        .enumerate()
        .map(|(id, (color, value))| Card {
            id: id as u8,
            color,
            value,
        })
        .collect()
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Phase {
    Turn,
    Drawn {
        card: u8,
    },
    OpeningColor,
    Uno {
        player: usize,
    },
    Challenge {
        sender: usize,
        recipient: usize,
    },
    Swap {
        player: usize,
    },
    TouchFive {
        player: usize,
        challenge: u64,
        starts_at: u64,
        taps: Vec<usize>,
    },
    Finished {
        winner: Option<usize>,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
enum Effect {
    Advance(usize),
    DrawTwo,
    Zero,
    Five,
    Four {
        sender: usize,
        evidence: Vec<Card>,
        illegal: bool,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Pending {
    effect: Effect,
    provisional: Option<usize>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Game {
    rules_version: String,
    hands: Vec<Vec<Card>>,
    draw: Vec<Card>,
    discard: Vec<Card>,
    color: Color,
    direction: i8,
    dealer: usize,
    turn: usize,
    phase: Phase,
    deadline: u64,
    paused_remaining: Option<u64>,
    pending: Option<Pending>,
    rng: u64,
    sequences: Vec<u64>,
    last_time: u64,
    stalled: usize,
    pile_empty: bool,
    #[serde(default)]
    reaction_serial: u64,
    #[serde(default)]
    last_touch: Option<TouchResult>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TouchResult {
    pub challenge: u64,
    pub order: Vec<usize>,
    pub missed: Vec<usize>,
    pub penalty: usize,
    pub resolved_at: u64,
    #[serde(default)]
    pub drawn: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Action {
    Play {
        card: u8,
        color: Option<Color>,
        uno: bool,
    },
    PlayNumbers {
        cards: Vec<u8>,
        uno: bool,
    },
    SwapHands {
        target: usize,
    },
    KeepHand,
    TouchFive {
        challenge: u64,
    },
    Draw,
    Keep,
    ChooseColor(Color),
    CallUno,
    CatchUno,
    AcceptFour,
    ChallengeFour,
}
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("Choose between 2 and 10 seats")]
    Seats,
    #[error("This seat does not exist")]
    Seat,
    #[error("Command already accepted or superseded")]
    Duplicate,
    #[error("Game is paused")]
    Paused,
    #[error("Resolve the server deadline first")]
    Deadline,
    #[error("Clock moved backwards")]
    Clock,
    #[error("Action is unavailable in this phase")]
    Phase,
    #[error("It is another player's turn")]
    Turn,
    #[error("Card is not playable")]
    Card,
    #[error("Choose a color for a Wild card")]
    Color,
    #[error("Could not obtain operating-system randomness")]
    Entropy,
}
/// Returned only to the command sender. Never broadcast challenge evidence.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Receipt {
    pub challenge_evidence: Option<Vec<Card>>,
    pub challenge_succeeded: Option<bool>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct View {
    pub rules_version: String,
    pub hand: Vec<Card>,
    pub counts: Vec<usize>,
    pub top: Card,
    pub color: Color,
    pub direction: i8,
    pub dealer: usize,
    pub turn: usize,
    pub phase: Phase,
    pub deadline: Option<u64>,
    pub paused: bool,
    pub draw_count: usize,
    pub playable: Vec<u8>,
    pub four_warning: bool,
    pub pile_empty: bool,
    #[serde(default)]
    pub touch_ready: bool,
    #[serde(default)]
    pub last_touch: Option<TouchResult>,
}

fn random(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9e3779b97f4a7c15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
    z ^ (z >> 31)
}
fn shuffle(cards: &mut [Card], rng: &mut u64) {
    for i in (1..cards.len()).rev() {
        let j = (random(rng) % (i as u64 + 1)) as usize;
        cards.swap(i, j);
    }
}

impl Game {
    /// Restore deadlines after host downtime without consuming offline decision time.
    /// `saved_at` is the host timestamp committed atomically with this game.
    pub fn rebase_clock(&mut self, saved_at: u64, now: u64) -> Result<(), Error> {
        if saved_at < self.last_time {
            return Err(Error::Clock);
        }
        self.deadline = now.saturating_add(self.deadline.saturating_sub(saved_at));
        if let Phase::TouchFive {
            ref mut starts_at, ..
        } = self.phase
        {
            *starts_at = if *starts_at >= saved_at {
                now.saturating_add(*starts_at - saved_at)
            } else {
                now.saturating_sub(saved_at - *starts_at)
            };
        }
        self.last_time = now;
        Ok(())
    }

    pub fn accepted_sequence(&self, seat: usize) -> Result<u64, Error> {
        self.sequences.get(seat).copied().ok_or(Error::Seat)
    }
    pub fn new(seats: usize, now: u64) -> Result<Self, Error> {
        let mut bytes = [0; 8];
        getrandom::getrandom(&mut bytes).map_err(|_| Error::Entropy)?;
        Self::seeded(seats, u64::from_le_bytes(bytes), now)
    }
    /// Deterministic fixtures/replays only; production hosts should call `new`.
    pub fn seeded(seats: usize, seed: u64, now: u64) -> Result<Self, Error> {
        if !(2..=10).contains(&seats) {
            return Err(Error::Seats);
        }
        let mut rng = seed;
        let mut draw = deck();
        shuffle(&mut draw, &mut rng);
        let dealer = (random(&mut rng) % seats as u64) as usize;
        let mut hands = vec![Vec::new(); seats];
        for _ in 0..7 {
            for offset in 1..=seats {
                hands[(dealer + offset) % seats].push(draw.pop().unwrap());
            }
        }
        let mut rejected = Vec::new();
        let top = loop {
            let c = draw.pop().unwrap();
            if c.value != Value::WildFour {
                break c;
            }
            rejected.push(c);
        };
        draw.extend(rejected);
        shuffle(&mut draw, &mut rng);
        let mut g = Self {
            rules_version: RULES_VERSION.into(),
            hands,
            draw,
            discard: vec![top],
            color: top.color.unwrap_or(Color::Red),
            direction: 1,
            dealer,
            turn: (dealer + 1) % seats,
            phase: Phase::Turn,
            deadline: now.saturating_add(TURN_MS),
            paused_remaining: None,
            pending: None,
            rng,
            sequences: vec![0; seats],
            last_time: now,
            stalled: 0,
            pile_empty: false,
            reaction_serial: 0,
            last_touch: None,
        };
        match top.value {
            Value::Skip => g.turn = g.next(g.turn, 1),
            Value::Reverse => {
                g.direction = -1;
                g.turn = dealer;
            }
            Value::DrawTwo => {
                g.take(g.turn, 2);
                g.turn = g.next(g.turn, 1);
            }
            Value::Wild => g.phase = Phase::OpeningColor,
            _ => {}
        }
        Ok(g)
    }
    fn next(&self, seat: usize, steps: usize) -> usize {
        (seat as isize + self.direction as isize * steps as isize)
            .rem_euclid(self.hands.len() as isize) as usize
    }
    fn take(&mut self, seat: usize, count: usize) -> Vec<Card> {
        let mut taken = Vec::new();
        for _ in 0..count {
            if self.draw.is_empty() && self.discard.len() > 1 {
                let top = self.discard.pop().unwrap();
                self.draw = std::mem::replace(&mut self.discard, vec![top]);
                shuffle(&mut self.draw, &mut self.rng);
            }
            if let Some(card) = self.draw.pop() {
                self.hands[seat].push(card);
                taken.push(card);
            } else {
                self.pile_empty = true;
                break;
            }
        }
        taken
    }
    fn matches(&self, card: Card) -> bool {
        card.color.is_none()
            || card.color == Some(self.color)
            || card.value == self.discard.last().unwrap().value
    }
    pub fn view(&self, seat: usize) -> Result<View, Error> {
        let hand = self.hands.get(seat).ok_or(Error::Seat)?.clone();
        let actionable = seat == self.turn && self.paused_remaining.is_none();
        let playable = hand
            .iter()
            .filter(|c| {
                actionable
                    && self.matches(**c)
                    && match self.phase {
                        Phase::Turn => true,
                        Phase::Drawn { card } => c.id == card,
                        _ => false,
                    }
            })
            .map(|c| c.id)
            .collect();
        Ok(View {
            rules_version: self.rules_version.clone(),
            counts: self.hands.iter().map(Vec::len).collect(),
            top: *self.discard.last().unwrap(),
            color: self.color,
            direction: self.direction,
            dealer: self.dealer,
            turn: self.turn,
            phase: self.phase.clone(),
            deadline: if self.paused_remaining.is_some()
                || matches!(self.phase, Phase::Finished { .. })
            {
                None
            } else {
                Some(self.deadline)
            },
            paused: self.paused_remaining.is_some(),
            draw_count: self.draw.len(),
            playable,
            four_warning: hand.iter().any(|c| c.color == Some(self.color)),
            hand,
            pile_empty: self.pile_empty,
            touch_ready: matches!(self.phase, Phase::TouchFive { starts_at, .. } if self.last_time>=starts_at),
            last_touch: self.last_touch.clone(),
        })
    }
    /// Accepted command sequences are strictly increasing per authenticated seat.
    /// Invalid commands never mutate the state; duplicate accepted commands never execute twice.
    pub fn apply(
        &mut self,
        seat: usize,
        sequence: u64,
        action: Action,
        now: u64,
    ) -> Result<Receipt, Error> {
        if seat >= self.hands.len() {
            return Err(Error::Seat);
        }
        if sequence <= self.sequences[seat] {
            return Err(Error::Duplicate);
        }
        if now < self.last_time {
            return Err(Error::Clock);
        }
        if self.paused_remaining.is_some() {
            return Err(Error::Paused);
        }
        if now >= self.deadline {
            return Err(Error::Deadline);
        }
        let mut draft = self.clone();
        let receipt = draft.act(seat, action, now)?;
        draft.sequences[seat] = sequence;
        draft.last_time = now;
        *self = draft;
        Ok(receipt)
    }
    fn act(&mut self, seat: usize, action: Action, now: u64) -> Result<Receipt, Error> {
        let mut receipt = Receipt::default();
        if let Phase::TouchFive {
            challenge,
            starts_at,
            ref taps,
            ..
        } = self.phase
        {
            if !matches!(action, Action::TouchFive { challenge: id } if id==challenge)
                || now < starts_at
            {
                return Err(Error::Phase);
            }
            if taps.contains(&seat) {
                return Err(Error::Duplicate);
            }
            if let Phase::TouchFive { ref mut taps, .. } = self.phase {
                taps.push(seat);
            }
            if matches!(&self.phase, Phase::TouchFive { taps, .. } if taps.len()==self.hands.len())
            {
                self.finish_touch(now);
            }
            return Ok(receipt);
        }
        if let Phase::Uno { player } = self.phase {
            match action {
                Action::CallUno if seat == player => {}
                Action::CatchUno if seat != player => {
                    self.take(player, 2);
                }
                _ => return Err(Error::Phase),
            }
            self.resolve(now);
            return Ok(receipt);
        }
        if seat != self.turn {
            return Err(Error::Turn);
        }
        match (self.phase.clone(), action) {
            (Phase::Turn, Action::PlayNumbers { cards, uno }) => {
                if self.rules_version != RULES_VERSION || cards.len() < 2 {
                    return Err(Error::Card);
                }
                let mut played = Vec::new();
                for id in &cards {
                    if played.iter().any(|c: &Card| c.id == *id) {
                        return Err(Error::Card);
                    }
                    played.push(
                        *self.hands[seat]
                            .iter()
                            .find(|c| c.id == *id)
                            .ok_or(Error::Card)?,
                    );
                }
                let Value::Number(number) = played[0].value else {
                    return Err(Error::Card);
                };
                if !played.iter().any(|card| self.matches(*card))
                    || played.iter().any(|c| c.value != Value::Number(number))
                {
                    return Err(Error::Card);
                }
                for card in played {
                    self.hands[seat].retain(|c| c.id != card.id);
                    self.color = card.color.unwrap();
                    self.discard.push(card);
                }
                self.stalled = 0;
                self.pending = Some(Pending {
                    effect: self.number_effect(number),
                    provisional: self.hands[seat].is_empty().then_some(seat),
                });
                self.after_play(seat, uno, now);
            }
            (Phase::Swap { player }, Action::KeepHand) if player == seat => self.end_swap(now),
            (Phase::Swap { player }, Action::SwapHands { target }) if player == seat => {
                if target >= self.hands.len() || target == seat {
                    return Err(Error::Seat);
                }
                self.hands.swap(seat, target);
                self.end_swap(now);
            }
            (Phase::Turn | Phase::Drawn { .. }, Action::Play { card, color, uno }) => {
                if let Phase::Drawn { card: drawn } = self.phase {
                    if drawn != card {
                        return Err(Error::Card);
                    }
                }
                let index = self.hands[seat]
                    .iter()
                    .position(|c| c.id == card)
                    .ok_or(Error::Card)?;
                let played = self.hands[seat][index];
                if !self.matches(played) {
                    return Err(Error::Card);
                }
                if played.color.is_none() != color.is_some() {
                    return Err(Error::Color);
                }
                let evidence = self.hands[seat].clone();
                let illegal = evidence.iter().any(|c| c.color == Some(self.color));
                self.hands[seat].remove(index);
                self.discard.push(played);
                self.color = played.color.or(color).unwrap();
                self.stalled = 0;
                let effect = match played.value {
                    Value::Reverse => {
                        self.direction *= -1;
                        Effect::Advance(if self.hands.len() == 2 { 2 } else { 1 })
                    }
                    Value::Skip => Effect::Advance(2),
                    Value::DrawTwo => Effect::DrawTwo,
                    Value::WildFour => Effect::Four {
                        sender: seat,
                        evidence,
                        illegal,
                    },
                    Value::Number(number) => self.number_effect(number),
                    _ => Effect::Advance(1),
                };
                self.pending = Some(Pending {
                    effect,
                    provisional: self.hands[seat].is_empty().then_some(seat),
                });
                self.after_play(seat, uno, now);
            }
            (Phase::Turn, Action::Draw) => {
                let taken = self.take(seat, 1);
                if let Some(card) = taken.first() {
                    self.stalled = 0;
                    if self.matches(*card) {
                        self.phase = Phase::Drawn { card: card.id };
                    } else {
                        self.advance(1, now);
                    }
                } else {
                    self.stalled += 1;
                    if self.stalled >= self.hands.len() {
                        self.phase = Phase::Finished { winner: None };
                    } else {
                        self.advance(1, now);
                    }
                }
            }
            (Phase::Drawn { .. }, Action::Keep) => self.advance(1, now),
            (Phase::OpeningColor, Action::ChooseColor(color)) => {
                self.color = color;
                self.phase = Phase::Turn;
            }
            (Phase::Challenge { sender, recipient }, Action::AcceptFour) => {
                self.take(recipient, 4);
                self.finish_or_advance(sender, recipient, now);
            }
            (Phase::Challenge { sender, recipient }, Action::ChallengeFour) => {
                let Some(Pending {
                    effect:
                        Effect::Four {
                            evidence, illegal, ..
                        },
                    ..
                }) = self.pending.clone()
                else {
                    return Err(Error::Phase);
                };
                receipt.challenge_evidence = Some(evidence);
                receipt.challenge_succeeded = Some(illegal);
                if illegal {
                    self.take(sender, 4);
                    self.pending = None;
                    self.turn = recipient;
                    self.phase = Phase::Turn;
                    self.deadline = now.saturating_add(TURN_MS);
                } else {
                    self.take(recipient, 6);
                    self.finish_or_advance(sender, recipient, now);
                }
            }
            _ => return Err(Error::Phase),
        }
        Ok(receipt)
    }
    fn advance(&mut self, steps: usize, now: u64) {
        self.turn = self.next(self.turn, steps);
        self.phase = Phase::Turn;
        self.deadline = now.saturating_add(TURN_MS);
    }
    fn number_effect(&self, number: u8) -> Effect {
        if self.rules_version == RULES_VERSION {
            match number {
                0 => Effect::Zero,
                5 => Effect::Five,
                _ => Effect::Advance(1),
            }
        } else {
            Effect::Advance(1)
        }
    }
    fn after_play(&mut self, seat: usize, uno: bool, now: u64) {
        if self.hands[seat].len() == 1 && !uno {
            self.phase = Phase::Uno { player: seat };
            self.deadline = now.saturating_add(UNO_MS);
        } else {
            self.resolve(now);
        }
    }
    fn end_swap(&mut self, now: u64) {
        self.pending = None;
        self.advance(1, now);
    }
    fn finish_touch(&mut self, now: u64) {
        let Phase::TouchFive {
            player,
            challenge,
            taps,
            ..
        } = self.phase.clone()
        else {
            return;
        };
        let missed: Vec<_> = (0..self.hands.len())
            .filter(|s| !taps.contains(s))
            .collect();
        let penalty = if missed.is_empty() {
            *taps.last().unwrap()
        } else {
            missed[(random(&mut self.rng) % missed.len() as u64) as usize]
        };
        let drawn = self.take(penalty, 1).len();
        self.last_touch = Some(TouchResult {
            challenge,
            order: taps,
            missed,
            penalty,
            resolved_at: now,
            drawn,
        });
        self.pending = None;
        self.turn = player;
        if self.hands[player].is_empty() {
            self.phase = Phase::Finished {
                winner: Some(player),
            };
        } else {
            self.advance(1, now);
        }
    }
    fn finish_or_advance(&mut self, sender: usize, recipient: usize, now: u64) {
        self.pending = None;
        if self.hands[sender].is_empty() {
            self.phase = Phase::Finished {
                winner: Some(sender),
            };
        } else {
            self.turn = recipient;
            self.advance(1, now);
        }
    }
    fn resolve(&mut self, now: u64) {
        let pending = self
            .pending
            .clone()
            .expect("pending effect during UNO response");
        match pending.effect {
            Effect::Zero if pending.provisional.is_none() => {
                self.phase = Phase::Swap { player: self.turn };
                self.deadline = now.saturating_add(SWAP_MS);
            }
            Effect::Five => {
                self.reaction_serial = self.reaction_serial.saturating_add(1);
                let starts_at = now.saturating_add(FIVE_READY_MS);
                self.phase = Phase::TouchFive {
                    player: self.turn,
                    challenge: self.reaction_serial,
                    starts_at,
                    taps: vec![],
                };
                self.deadline = starts_at.saturating_add(FIVE_MS);
            }
            Effect::Four { sender, .. } => {
                let recipient = self.next(sender, 1);
                self.turn = recipient;
                self.phase = Phase::Challenge { sender, recipient };
                self.deadline = now.saturating_add(CHALLENGE_MS);
            }
            effect => {
                match effect {
                    Effect::DrawTwo => {
                        self.take(self.next(self.turn, 1), 2);
                    }
                    Effect::Advance(_) | Effect::Zero => {}
                    _ => unreachable!(),
                }
                self.pending = None;
                if let Some(winner) = pending.provisional {
                    self.phase = Phase::Finished {
                        winner: Some(winner),
                    };
                } else {
                    self.advance(
                        match effect {
                            Effect::Advance(n) => n,
                            _ => 2,
                        },
                        now,
                    );
                }
            }
        }
    }
    /// Host invokes before commands when the saved deadline is due. No work occurs while paused.
    pub fn tick(&mut self, now: u64) -> Result<bool, Error> {
        if now < self.last_time {
            return Err(Error::Clock);
        }
        self.last_time = now;
        if self.paused_remaining.is_some()
            || now < self.deadline
            || matches!(self.phase, Phase::Finished { .. })
        {
            return Ok(false);
        }
        match self.phase {
            Phase::Uno { .. } => self.resolve(now),
            Phase::Challenge { .. } => {
                self.act(self.turn, Action::AcceptFour, now)?;
            }
            Phase::Turn => {
                self.act(self.turn, Action::Draw, now)?;
                if matches!(self.phase, Phase::Drawn { .. }) {
                    self.advance(1, now);
                }
            }
            Phase::Drawn { .. } => self.advance(1, now),
            Phase::OpeningColor => {
                self.color = Color::Red;
                self.phase = Phase::Turn;
                self.deadline = now.saturating_add(TURN_MS);
            }
            Phase::Swap { .. } => self.end_swap(now),
            Phase::TouchFive { .. } => self.finish_touch(now),
            Phase::Finished { .. } => {}
        }
        Ok(true)
    }
    /// Host owns pause-vote authorization. Resolve due deadlines before freezing.
    pub fn pause(&mut self, now: u64) -> Result<(), Error> {
        self.tick(now)?;
        if self.paused_remaining.is_none() {
            self.paused_remaining = Some(self.deadline.saturating_sub(now));
        }
        Ok(())
    }
    pub fn resume(&mut self, now: u64) -> Result<(), Error> {
        if now < self.last_time {
            return Err(Error::Clock);
        }
        if let Some(remaining) = self.paused_remaining.take() {
            self.deadline = now.saturating_add(remaining);
            if let Phase::TouchFive {
                ref mut starts_at, ..
            } = self.phase
            {
                *starts_at = now.saturating_add(remaining.saturating_sub(FIVE_MS));
            }
        }
        self.last_time = now;
        Ok(())
    }
}

/// Fair-information policy: accepts only a player view, never authoritative state.
pub fn number_group_playable(view: &View, cards: &[u8]) -> bool {
    if view.rules_version != RULES_VERSION
        || view.paused
        || !matches!(view.phase, Phase::Turn)
        || cards.is_empty()
        || !cards.iter().any(|id| view.playable.contains(id))
    {
        return false;
    }
    let Some(first) = view.hand.iter().find(|c| c.id == cards[0]) else {
        return false;
    };
    let Value::Number(number) = first.value else {
        return false;
    };
    cards.iter().enumerate().all(|(i, id)| {
        !cards[..i].contains(id)
            && view
                .hand
                .iter()
                .any(|c| c.id == *id && c.value == Value::Number(number))
    })
}

/// Fair-information policy: accepts only a player view, never authoritative state.
pub fn bot_action(view: &View, seat: usize) -> Option<Action> {
    if view.paused {
        return None;
    }
    if let Phase::TouchFive {
        challenge,
        ref taps,
        ..
    } = view.phase
    {
        return (view.touch_ready && !taps.contains(&seat))
            .then_some(Action::TouchFive { challenge });
    }
    if let Phase::Uno { player } = view.phase {
        return (player == seat).then_some(Action::CallUno);
    }
    if view.turn != seat {
        return None;
    }
    let color = Color::ALL
        .into_iter()
        .max_by_key(|color| view.hand.iter().filter(|c| c.color == Some(*color)).count())
        .unwrap();
    match view.phase {
        Phase::OpeningColor => Some(Action::ChooseColor(color)),
        Phase::Challenge { .. } => Some(Action::AcceptFour),
        Phase::Swap { .. } => {
            let target = (0..view.counts.len())
                .filter(|s| *s != seat)
                .min_by_key(|s| view.counts[*s]);
            Some(match target {
                Some(target) if view.counts[target] < view.hand.len() => {
                    Action::SwapHands { target }
                }
                _ => Action::KeepHand,
            })
        }
        Phase::Turn | Phase::Drawn { .. } => {
            let card = view
                .hand
                .iter()
                .filter(|c| {
                    view.playable.contains(&c.id)
                        && !(c.value == Value::WildFour && view.four_warning)
                })
                .max_by_key(|c| match c.value {
                    Value::WildFour => 0,
                    Value::Wild => 1,
                    Value::Number(n) => n as u16 + 2,
                    _ => 12,
                });
            Some(if let Some(card) = card {
                let group: Vec<_> = std::iter::once(card.id)
                    .chain(
                        view.hand
                            .iter()
                            .filter(|c| c.id != card.id && c.value == card.value)
                            .map(|c| c.id),
                    )
                    .collect();
                if matches!(card.value, Value::Number(_))
                    && group.len() > 1
                    && number_group_playable(view, &group)
                {
                    Action::PlayNumbers {
                        cards: group,
                        uno: true,
                    }
                } else {
                    Action::Play {
                        card: card.id,
                        color: card.color.is_none().then_some(color),
                        uno: true,
                    }
                }
            } else if matches!(view.phase, Phase::Drawn { .. }) {
                Action::Keep
            } else {
                Action::Draw
            })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests;
