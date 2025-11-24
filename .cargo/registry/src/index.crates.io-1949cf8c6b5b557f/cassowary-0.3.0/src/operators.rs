use ::std::ops;
use {
    Term,
    Variable,
    Expression,
    WeightedRelation,
    PartialConstraint,
    Constraint
};

// Relation

impl ops::BitOr<WeightedRelation> for f64 {
    type Output = PartialConstraint;
    fn bitor(self, r: WeightedRelation) -> PartialConstraint {
        PartialConstraint(self.into(), r)
    }
}
impl ops::BitOr<WeightedRelation> for f32 {
    type Output = PartialConstraint;
    fn bitor(self, r: WeightedRelation) -> PartialConstraint {
        (self as f64).bitor(r)
    }
}
impl ops::BitOr<WeightedRelation> for Variable {
    type Output = PartialConstraint;
    fn bitor(self, r: WeightedRelation) -> PartialConstraint {
        PartialConstraint(self.into(), r)
    }
}
impl ops::BitOr<WeightedRelation> for Term {
    type Output = PartialConstraint;
    fn bitor(self, r: WeightedRelation) -> PartialConstraint {
        PartialConstraint(self.into(), r)
    }
}
impl ops::BitOr<WeightedRelation> for Expression {
    type Output = PartialConstraint;
    fn bitor(self, r: WeightedRelation) -> PartialConstraint {
        PartialConstraint(self.into(), r)
    }
}

impl ops::BitOr<f64> for PartialConstraint {
    type Output = Constraint;
    fn bitor(self, rhs: f64) -> Constraint {
        let (op, s) = self.1.into();
        Constraint::new(self.0 - rhs, op, s)
    }
}
impl ops::BitOr<f32> for PartialConstraint {
    type Output = Constraint;
    fn bitor(self, rhs: f32) -> Constraint {
        self.bitor(rhs as f64)
    }
}
impl ops::BitOr<Variable> for PartialConstraint {
    type Output = Constraint;
    fn bitor(self, rhs: Variable) -> Constraint {
        let (op, s) = self.1.into();
        Constraint::new(self.0 - rhs, op, s)
    }
}
impl ops::BitOr<Term> for PartialConstraint {
    type Output = Constraint;
    fn bitor(self, rhs: Term) -> Constraint {
        let (op, s) = self.1.into();
        Constraint::new(self.0 - rhs, op, s)
    }
}
impl ops::BitOr<Expression> for PartialConstraint {
    type Output = Constraint;
    fn bitor(self, rhs: Expression) -> Constraint {
        let (op, s) = self.1.into();
        Constraint::new(self.0 - rhs, op, s)
    }
}

// Variable

impl ops::Add<f64> for Variable {
    type Output = Expression;
    fn add(self, v: f64) -> Expression {
        Expression::new(vec![Term::new(self, 1.0)], v)
    }
}

impl ops::Add<f32> for Variable {
    type Output = Expression;
    fn add(self, v: f32) -> Expression {
        self.add(v as f64)
    }
}

impl ops::Add<Variable> for f64 {
    type Output = Expression;
    fn add(self, v: Variable) -> Expression {
        Expression::new(vec![Term::new(v, 1.0)], self)
    }
}

impl ops::Add<Variable> for f32 {
    type Output = Expression;
    fn add(self, v: Variable) -> Expression {
        (self as f64).add(v)
    }
}

impl ops::Add<Variable> for Variable {
    type Output = Expression;
    fn add(self, v: Variable) -> Expression {
        Expression::new(vec![Term::new(self, 1.0), Term::new(v, 1.0)], 0.0)
    }
}

impl ops::Add<Term> for Variable {
    type Output = Expression;
    fn add(self, t: Term) -> Expression {
        Expression::new(vec![Term::new(self, 1.0), t], 0.0)
    }
}

impl ops::Add<Variable> for Term {
    type Output = Expression;
    fn add(self, v: Variable) -> Expression {
        Expression::new(vec![self, Term::new(v, 1.0)], 0.0)
    }
}

impl ops::Add<Expression> for Variable {
    type Output = Expression;
    fn add(self, mut e: Expression) -> Expression {
        e.terms.push(Term::new(self, 1.0));
        e
    }
}

impl ops::Add<Variable> for Expression {
    type Output = Expression;
    fn add(mut self, v: Variable) -> Expression {
        self.terms.push(Term::new(v, 1.0));
        self
    }
}

impl ops::Neg for Variable {
    type Output = Term;
    fn neg(self) -> Term {
        Term::new(self, -1.0)
    }
}

impl ops::Sub<f64> for Variable {
    type Output = Expression;
    fn sub(self, v: f64) -> Expression {
        Expression::new(vec![Term::new(self, 1.0)], -v)
    }
}

impl ops::Sub<f32> for Variable {
    type Output = Expression;
    fn sub(self, v: f32) -> Expression {
        self.sub(v as f64)
    }
}

impl ops::Sub<Variable> for f64 {
    type Output = Expression;
    fn sub(self, v: Variable) -> Expression {
        Expression::new(vec![Term::new(v, -1.0)], self)
    }
}

impl ops::Sub<Variable> for f32 {
    type Output = Expression;
    fn sub(self, v: Variable) -> Expression {
        (self as f64).sub(v)
    }
}

impl ops::Sub<Variable> for Variable {
    type Output = Expression;
    fn sub(self, v: Variable) -> Expression {
        Expression::new(vec![Term::new(self, 1.0), Term::new(v, -1.0)], 0.0)
    }
}

impl ops::Sub<Term> for Variable {
    type Output = Expression;
    fn sub(self, t: Term) -> Expression {
        Expression::new(vec![Term::new(self, 1.0), -t], 0.0)
    }
}

impl ops::Sub<Variable> for Term {
    type Output = Expression;
    fn sub(self, v: Variable) -> Expression {
        Expression::new(vec![self, Term::new(v, -1.0)], 0.0)
    }
}

impl ops::Sub<Expression> for Variable {
    type Output = Expression;
    fn sub(self, mut e: Expression) -> Expression {
        e.negate();
        e.terms.push(Term::new(self, 1.0));
        e
    }
}

impl ops::Sub<Variable> for Expression {
    type Output = Expression;
    fn sub(mut self, v: Variable) -> Expression {
        self.terms.push(Term::new(v, -1.0));
        self
    }
}

impl ops::Mul<f64> for Variable {
    type Output = Term;
    fn mul(self, v: f64) -> Term {
        Term::new(self, v)
    }
}

impl ops::Mul<f32> for Variable {
    type Output = Term;
    fn mul(self, v: f32) -> Term {
        self.mul(v as f64)
    }
}

impl ops::Mul<Variable> for f64 {
    type Output = Term;
    fn mul(self, v: Variable) -> Term {
        Term::new(v, self)
    }
}

impl ops::Mul<Variable> for f32 {
    type Output = Term;
    fn mul(self, v: Variable) -> Term {
        (self as f64).mul(v)
    }
}

impl ops::Div<f64> for Variable {
    type Output = Term;
    fn div(self, v: f64) -> Term {
        Term::new(self, 1.0 / v)
    }
}

impl ops::Div<f32> for Variable {
    type Output = Term;
    fn div(self, v: f32) -> Term {
        self.div(v as f64)
    }
}

// Term

impl ops::Mul<f64> for Term {
    type Output = Term;
    fn mul(mut self, v: f64) -> Term {
        self.coefficient *= v;
        self
    }
}

impl ops::Mul<f32> for Term {
    type Output = Term;
    fn mul(self, v: f32) -> Term {
        self.mul(v as f64)
    }
}

impl ops::Mul<Term> for f64 {
    type Output = Term;
    fn mul(self, mut t: Term) -> Term {
        t.coefficient *= self;
        t
    }
}

impl ops::Mul<Term> for f32 {
    type Output = Term;
    fn mul(self, t: Term) -> Term {
        (self as f64).mul(t)
    }
}

impl ops::Div<f64> for Term {
    type Output = Term;
    fn div(mut self, v: f64) -> Term {
        self.coefficient /= v;
        self
    }
}

impl ops::Div<f32> for Term {
    type Output = Term;
    fn div(self, v: f32) -> Term {
        self.div(v as f64)
    }
}

impl ops::Add<f64> for Term {
    type Output = Expression;
    fn add(self, v: f64) -> Expression {
        Expression::new(vec![self], v)
    }
}

impl ops::Add<f32> for Term {
    type Output = Expression;
    fn add(self, v: f32) -> Expression {
        self.add(v as f64)
    }
}

impl ops::Add<Term> for f64 {
    type Output = Expression;
    fn add(self, t: Term) -> Expression {
        Expression::new(vec![t], self)
    }
}

impl ops::Add<Term> for f32 {
    type Output = Expression;
    fn add(self, t: Term) -> Expression {
        (self as f64).add(t)
    }
}

impl ops::Add<Term> for Term {
    type Output = Expression;
    fn add(self, t: Term) -> Expression {
        Expression::new(vec![self, t], 0.0)
    }
}

impl ops::Add<Expression> for Term {
    type Output = Expression;
    fn add(self, mut e: Expression) -> Expression {
        e.terms.push(self);
        e
    }
}

impl ops::Add<Term> for Expression {
    type Output = Expression;
    fn add(mut self, t: Term) -> Expression {
        self.terms.push(t);
        self
    }
}

impl ops::Neg for Term {
    type Output = Term;
    fn neg(mut self) -> Term {
        self.coefficient = -self.coefficient;
        self
    }
}

impl ops::Sub<f64> for Term {
    type Output = Expression;
    fn sub(self, v: f64) -> Expression {
        Expression::new(vec![self], -v)
    }
}

impl ops::Sub<f32> for Term {
    type Output = Expression;
    fn sub(self, v: f32) -> Expression {
        self.sub(v as f64)
    }
}

impl ops::Sub<Term> for f64 {
    type Output = Expression;
    fn sub(self, t: Term) -> Expression {
        Expression::new(vec![-t], self)
    }
}

impl ops::Sub<Term> for f32 {
    type Output = Expression;
    fn sub(self, t: Term) -> Expression {
        (self as f64).sub(t)
    }
}

impl ops::Sub<Term> for Term {
    type Output = Expression;
    fn sub(self, t: Term) -> Expression {
        Expression::new(vec![self, -t], 0.0)
    }
}

impl ops::Sub<Expression> for Term {
    type Output = Expression;
    fn sub(self, mut e: Expression) -> Expression {
        e.negate();
        e.terms.push(self);
        e
    }
}

impl ops::Sub<Term> for Expression {
    type Output = Expression;
    fn sub(mut self, t: Term) -> Expression {
        self.terms.push(-t);
        self
    }
}

// Expression

impl ops::Mul<f64> for Expression {
    type Output = Expression;
    fn mul(mut self, v: f64) -> Expression {
        self.constant *= v;
        for t in &mut self.terms {
            *t = *t * v;
        }
        self
    }
}

impl ops::Mul<f32> for Expression {
    type Output = Expression;
    fn mul(self, v: f32) -> Expression {
        self.mul(v as f64)
    }
}

impl ops::Mul<Expression> for f64 {
    type Output = Expression;
    fn mul(self, mut e: Expression) -> Expression {
        e.constant *= self;
        for t in &mut e.terms {
            *t = *t * self;
        }
        e
    }
}

impl ops::Mul<Expression> for f32 {
    type Output = Expression;
    fn mul(self, e: Expression) -> Expression {
        (self as f64).mul(e)
    }
}

impl ops::Div<f64> for Expression {
    type Output = Expression;
    fn div(mut self, v: f64) -> Expression {
        self.constant /= v;
        for t in &mut self.terms {
            *t = *t / v;
        }
        self
    }
}

impl ops::Div<f32> for Expression {
    type Output = Expression;
    fn div(self, v: f32) -> Expression {
        self.div(v as f64)
    }
}

impl ops::Add<f64> for Expression {
    type Output = Expression;
    fn add(mut self, v: f64) -> Expression {
        self.constant += v;
        self
    }
}

impl ops::Add<f32> for Expression {
    type Output = Expression;
    fn add(self, v: f32) -> Expression {
        self.add(v as f64)
    }
}

impl ops::Add<Expression> for f64 {
    type Output = Expression;
    fn add(self, mut e: Expression) -> Expression {
        e.constant += self;
        e
    }
}

impl ops::Add<Expression> for f32 {
    type Output = Expression;
    fn add(self, e: Expression) -> Expression {
        (self as f64).add(e)
    }
}

impl ops::Add<Expression> for Expression {
    type Output = Expression;
    fn add(mut self, mut e: Expression) -> Expression {
        self.terms.append(&mut e.terms);
        self.constant += e.constant;
        self
    }
}

impl ops::Neg for Expression {
    type Output = Expression;
    fn neg(mut self) -> Expression {
        self.negate();
        self
    }
}

impl ops::Sub<f64> for Expression {
    type Output = Expression;
    fn sub(mut self, v: f64) -> Expression {
        self.constant -= v;
        self
    }
}

impl ops::Sub<f32> for Expression {
    type Output = Expression;
    fn sub(self, v: f32) -> Expression {
        self.sub(v as f64)
    }
}

impl ops::Sub<Expression> for f64 {
    type Output = Expression;
    fn sub(self, mut e: Expression) -> Expression {
        e.negate();
        e.constant += self;
        e
    }
}

impl ops::Sub<Expression> for f32 {
    type Output = Expression;
    fn sub(self, e: Expression) -> Expression {
        (self as f64).sub(e)
    }
}

impl ops::Sub<Expression> for Expression {
    type Output = Expression;
    fn sub(mut self, mut e: Expression) -> Expression {
        e.negate();
        self.terms.append(&mut e.terms);
        self.constant += e.constant;
        self
    }
}
