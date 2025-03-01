//! This module supports building and training models.
//!
//! This module currently requires the `experimental_training` feature.

use crate::ops;
use crate::DataType;
use crate::Operation;
use crate::Output;
use crate::Result;
use crate::Scope;
use crate::Tensor;
use crate::TensorType;
use crate::Variable;

/// Options for `Optimizer::minimize`.
#[derive(Default, Debug, Clone)]
pub struct MinimizeOptions<'a> {
    variables: &'a [Variable],
}

impl<'a> MinimizeOptions<'a> {
    /// Sets the variables which will be optimized.
    pub fn with_variables(self, variables: &'a [Variable]) -> Self {
        Self { variables }
    }
}

/// Options for `Optimizer::compute_gradients`.
#[derive(Default, Debug, Clone)]
pub struct ComputeGradientsOptions<'a> {
    variables: &'a [Variable],
}

impl<'a> ComputeGradientsOptions<'a> {
    /// Sets the variables whose gradients need to be computed.
    pub fn with_variables(self, variables: &'a [Variable]) -> Self {
        Self { variables }
    }
}

/// Options for `Optimizer::apply_gradients`.
#[derive(Default, Debug, Clone)]
pub struct ApplyGradientsOptions<'a> {
    grads_and_vars: &'a [(Option<Output>, Variable)],
}

impl<'a> ApplyGradientsOptions<'a> {
    /// Sets the variables which will be optimized and their associated gradients.
    pub fn with_grads_and_vars(self, grads_and_vars: &'a [(Option<Output>, Variable)]) -> Self {
        Self { grads_and_vars }
    }
}

/// An optimizer adjusts variables to minimize some specified value.
///
/// Basic usage only requires calling `minimize`, which calls
/// `compute_gradients` and `apply_gradients` internally.  Advanced users may
/// want to call `compute_gradients` and `apply_gradients` manually to allow
/// them to modify the gradients, e.g. for clipping.
pub trait Optimizer {
    /// Computes the gradient of a value with respect to the given variables.
    /// This adds nodes to the graph, so reuse its results if possible.
    /// Any variable whose gradient cannot be calculated will have a None gradient.
    ///
    /// Users are encouraged to call `minimize` instead unless they need to
    /// manually modify gradients.
    fn compute_gradients(
        &self,
        scope: &mut Scope,
        loss: Output,
        opts: ComputeGradientsOptions,
    ) -> Result<Vec<(Option<Output>, Variable)>> {
        let variable_outputs: Vec<_> = opts.variables.iter().map(|v| v.output.clone()).collect();
        let gradients = scope
            .graph_mut()
            .add_gradients(None, &[loss], &variable_outputs, None)?;
        let mut output = Vec::with_capacity(opts.variables.len());
        for (i, gradient) in gradients.into_iter().enumerate() {
            output.push((gradient, opts.variables[i].clone()));
        }
        Ok(output)
    }

    /// Applies the given gradients to the variables.
    ///
    /// This returns newly created variables which may be needed to track the
    /// optimizer's internal state, as well as an operation which applies the
    /// gradients once.
    ///
    /// Users are encouraged to call `minimize` instead unless they need to
    /// manually modify gradients.
    fn apply_gradients(
        &self,
        scope: &mut Scope,
        opts: ApplyGradientsOptions,
    ) -> Result<(Vec<Variable>, Operation)>;

    /// Adds operations to the graph to minimize loss with respect to the
    /// variables.
    ///
    /// This returns newly created variables which may be needed to track the
    /// optimizers internal state, as well as an operation which performs a
    /// single step of minimization.        
    fn minimize(
        &self,
        scope: &mut Scope,
        loss: Output,
        opts: MinimizeOptions,
    ) -> Result<(Vec<Variable>, Operation)> {
        let grads_and_vars = self.compute_gradients(
            scope,
            loss.clone(),
            ComputeGradientsOptions {
                variables: opts.variables,
            },
        )?;
        self.apply_gradients(
            scope,
            ApplyGradientsOptions {
                grads_and_vars: &grads_and_vars,
            },
        )
    }
}

/// Optimizer that implements the gradient descent algorithm.
#[derive(Debug)]
pub struct GradientDescentOptimizer {
    learning_rate: Output,
}

impl GradientDescentOptimizer {
    /// Creates a new optimizer with the given learning rate.
    pub fn new(learning_rate: Output) -> Self {
        Self { learning_rate }
    }
}

impl Optimizer for GradientDescentOptimizer {
    fn apply_gradients(
        &self,
        scope: &mut Scope,
        opts: ApplyGradientsOptions,
    ) -> Result<(Vec<Variable>, Operation)> {
        let mut apply_ops = Vec::new();
        for (grad, var) in opts.grads_and_vars {
            if let Some(grad) = grad {
                // TODO: use standard op
                let name = scope.get_unique_name_for_op("ApplyGradientDescent");
                let mut graph = scope.graph_mut();
                let mut nd = graph.new_operation("ApplyGradientDescent", &name)?;
                nd.add_input(var.output.clone());
                nd.add_input(self.learning_rate.clone());
                nd.add_input(grad.clone());
                apply_ops.push(nd.finish()?);
            }
        }
        let mut nop = ops::NoOp::new();
        for apply_op in &apply_ops {
            nop = nop.add_control_input(apply_op.clone());
        }
        Ok((Vec::new(), nop.build(scope)?))
    }
}

/// Optimizer that implements the Adadelta algorithm.
///
/// See [M. D. Zeiler](https://arxiv.org/abs/1212.5701).
#[derive(Debug)]
pub struct AdadeltaOptimizer {
    learning_rate: Option<Output>,
    rho: Option<Output>,
    epsilon: Option<Output>,
}

impl AdadeltaOptimizer {
    /// Creates a new optimizer with default parameters (learning_rate=0.001, rho=0.95, epsilon=1e-8).
    pub fn new() -> Self {
        Self {
            learning_rate: None,
            rho: None,
            epsilon: None,
        }
    }

    /// Sets the learning rate.  Default is 0.001.
    pub fn set_learning_rate<T: Into<Output>>(&mut self, learning_rate: T) {
        self.learning_rate = Some(learning_rate.into());
    }

    /// Sets rho, the decay rate.  Default is 0.95.
    pub fn set_rho<T: Into<Output>>(&mut self, rho: T) {
        self.rho = Some(rho.into());
    }

    /// Sets epsilon, the conditioning.  Default is 1e-8.
    pub fn set_epsilon<T: Into<Output>>(&mut self, epsilon: T) {
        self.epsilon = Some(epsilon.into());
    }
}

fn or_constant<T: TensorType, TT: Into<Tensor<T>>>(
    scope: &mut Scope,
    value: &Option<Output>,
    default: TT,
) -> Result<Output> {
    match value {
        Some(x) => Ok(x.clone()),
        None => Ok(ops::constant(scope, default)?.into()),
    }
}

fn create_zeros_slot(
    scope: &mut Scope,
    primary: &Variable,
    dtype: Option<DataType>,
) -> Result<Variable> {
    let dtype = dtype.unwrap_or_else(|| primary.dtype);
    // TODO: use standard op
    let zeros = {
        let name = scope.get_unique_name_for_op("ZerosLike");
        let mut graph = scope.graph_mut();
        let mut nd = graph.new_operation("ZerosLike", &name)?;
        nd.add_input(primary.output.clone());
        nd.add_control_input(&primary.initializer);
        nd.finish()?
    };
    Variable::builder()
        .initial_value(zeros)
        .shape(primary.shape.clone())
        .data_type(dtype)
        .build(scope)
}

impl Optimizer for AdadeltaOptimizer {
    fn apply_gradients(
        &self,
        scope: &mut Scope,
        opts: ApplyGradientsOptions,
    ) -> Result<(Vec<Variable>, Operation)> {
        let learning_rate = or_constant(scope, &self.learning_rate, 0.001f32)?;
        let rho = or_constant(scope, &self.rho, 0.95f32)?;
        let epsilon = or_constant(scope, &self.epsilon, 1e-8f32)?;
        let mut apply_ops = Vec::new();
        let mut variables = Vec::new();
        for (grad, var) in opts.grads_and_vars {
            if let Some(grad) = grad {
                let mut scope = scope.new_sub_scope(&var.name);
                let accum = create_zeros_slot(&mut scope.new_sub_scope("accum"), var, None)?;
                let accum_update =
                    create_zeros_slot(&mut scope.new_sub_scope("accum_update"), var, None)?;
                // TODO: use standard op
                let name = scope.get_unique_name_for_op("ApplyAdadelta");
                let mut graph = scope.graph_mut();
                let mut nd = graph.new_operation("ApplyAdadelta", &name)?;
                nd.add_input(var.output.clone());
                nd.add_input(accum.output.clone());
                nd.add_input(accum_update.output.clone());
                nd.add_input(learning_rate.clone());
                nd.add_input(rho.clone());
                nd.add_input(epsilon.clone());
                nd.add_input(grad.clone());
                apply_ops.push(nd.finish()?);
                variables.push(accum.clone());
                variables.push(accum_update.clone());
            }
        }
        let mut no_op = ops::NoOp::new();
        for apply_op in &apply_ops {
            no_op = no_op.add_control_input(apply_op.clone());
        }
        Ok((variables, no_op.build(scope)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Session;
    use crate::SessionOptions;
    use crate::SessionRunArgs;

    #[test]
    fn simple_gradient_descent() {
        let mut scope = Scope::new_root_scope();
        let x_var = Variable::builder()
            .const_initial_value::<_, f32>(3.0)
            .build(&mut scope.with_op_name("x"))
            .unwrap();
        let x_squared =
            ops::multiply(&mut scope, x_var.output.clone(), x_var.output.clone()).unwrap();
        let sgd = GradientDescentOptimizer {
            learning_rate: Output {
                operation: ops::constant(&mut scope, 0.1f32).unwrap(),
                index: 0,
            },
        };
        let (minimizer_vars, minimize) = sgd
            .minimize(
                &mut scope,
                x_squared.into(),
                MinimizeOptions::default().with_variables(&[x_var.clone()]),
            )
            .unwrap();
        let options = SessionOptions::new();
        let session = Session::new(&options, &scope.graph()).unwrap();

        let mut run_args = SessionRunArgs::new();
        run_args.add_target(&x_var.initializer);
        for var in &minimizer_vars {
            run_args.add_target(&var.initializer);
        }
        session.run(&mut run_args).unwrap();

        let mut run_args = SessionRunArgs::new();
        run_args.add_target(&minimize);
        let x_fetch = run_args.request_fetch(&x_var.output.operation, 0);

        session.run(&mut run_args).unwrap();
        let x_output = run_args.fetch::<f32>(x_fetch).unwrap();
        assert_eq!(x_output.len(), 1);
        assert!(
            x_output[0] >= 2.39 && x_output[0] <= 2.41,
            "x_output[0] = {}",
            x_output[0]
        );

        session.run(&mut run_args).unwrap();
        let x_output = run_args.fetch::<f32>(x_fetch).unwrap();
        assert_eq!(x_output.len(), 1);
        assert!(
            x_output[0] >= 1.91 && x_output[0] <= 1.93,
            "x_output[0] = {}",
            x_output[0]
        );

        session.run(&mut run_args).unwrap();
        let x_output = run_args.fetch::<f32>(x_fetch).unwrap();
        assert_eq!(x_output.len(), 1);
        assert!(
            x_output[0] >= 1.52 && x_output[0] <= 1.54,
            "x_output[0] = {}",
            x_output[0]
        );
    }

    #[test]
    fn simple_adadelta() {
        let mut scope = Scope::new_root_scope();
        let x_var = Variable::builder()
            .const_initial_value(3.0f32)
            .build(&mut scope.with_op_name("x"))
            .unwrap();
        let x_squared =
            ops::multiply(&mut scope, x_var.output.clone(), x_var.output.clone()).unwrap();
        let mut optimizer = AdadeltaOptimizer::new();
        optimizer.set_learning_rate(ops::constant(&mut scope, 0.1f32).unwrap());
        let (minimizer_vars, minimize) = optimizer
            .minimize(
                &mut scope,
                x_squared.into(),
                MinimizeOptions::default().with_variables(&[x_var.clone()]),
            )
            .unwrap();
        let options = SessionOptions::new();
        let session = Session::new(&options, &scope.graph()).unwrap();

        let mut run_args = SessionRunArgs::new();
        run_args.add_target(&x_var.initializer);
        for var in &minimizer_vars {
            run_args.add_target(&var.initializer);
        }
        session.run(&mut run_args).unwrap();

        let mut run_args = SessionRunArgs::new();
        run_args.add_target(&minimize);
        let x_fetch = run_args.request_fetch(&x_var.output.operation, 0);

        session.run(&mut run_args).unwrap();
        let x_output = run_args.fetch::<f32>(x_fetch).unwrap();
        assert_eq!(x_output.len(), 1);
        assert!(
            x_output[0] >= 2.99994 && x_output[0] <= 2.99996,
            "x_output[0] = {}",
            x_output[0]
        );

        session.run(&mut run_args).unwrap();
        let x_output = run_args.fetch::<f32>(x_fetch).unwrap();
        assert_eq!(x_output.len(), 1);
        assert!(
            x_output[0] >= 2.99990 && x_output[0] <= 2.99992,
            "x_output[0] = {}",
            x_output[0]
        );

        session.run(&mut run_args).unwrap();
        let x_output = run_args.fetch::<f32>(x_fetch).unwrap();
        assert_eq!(x_output.len(), 1);
        assert!(
            x_output[0] >= 2.99985 && x_output[0] <= 2.99987,
            "x_output[0] = {}",
            x_output[0]
        );
    }
}
