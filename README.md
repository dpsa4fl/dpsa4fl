
# dpsa4fl library

**Warning: This project is work in progress and should not be used in production. The current implementation is a prototype.**

The dpsa4fl project aims at providing a mechanism for secure and differentially private aggregation
of gradients in federated machine learning. For more information see the [project overview](https://github.com/dpsa-project/overview).

The [janus](https://github.com/divviup/janus) framework is used for secure aggregation. The dpsa4fl library
provides functionality for controller and clients to interact with a janus server instance. The janus server needs to be setup in a customized way,
see the [dpsa4fl infrastructure repo](https://github.com/dpsa-project/dpsa4fl-infrastructure) for instructions.
See our [example project](https://github.com/dpsa-project/dpsa4fl-example-project) for a description of how to setup an end-to-end test.


## Changelog

 - *Beta release* (2023-05-09): Using the library is now more user-friendly (stream-lined configuration, more error checking). Automatic error recovery when gradients contain malformed data. Three bitsizes are now available for the internal fixed-point representation (16, 32, 64 bit).
 - *Differential privacy* (2023-02-27): Gradient vectors are now noised by the aggregators, the amount of noise is configurable. The discrete gaussian distribution is used for sampling.
 - *Initial release* (2023-01-19): The dpsa4fl library can be used for aggregation of gradient vectors (without differential privacy).


## Funding

This project is funded through the [NGI Assure Fund](https://nlnet.nl/assure), a fund established by [NLnet](https://nlnet.nl) with financial support from the European Commission's [Next Generation Internet](https://ngi.eu) program. Learn more on the [NLnet project page](https://nlnet.nl/project/dist-mech-learn#ack).

[<img src="https://nlnet.nl/logo/banner.png" alt="NLnet foundation logo" width="20%" />](https://nlnet.nl)
[<img src="https://nlnet.nl/image/logos/NGIAssure_tag.svg" alt="NGI Assure Logo" width="20%" />](https://nlnet.nl/assure)

