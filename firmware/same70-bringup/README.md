# SAME70 Bringup

## Notes

* LEDs
    * PA05 - USER_LED0 - Active Low
* Button
    * PA11 - USER_BUTTON0 - Active Low (needs pullup?)

### Ethernet Pins

| Pin     | Name1           | Name2             |
| :---    | :---            | :---              |
| PD00    | GTXCK           | TXCK              |
| PD01    | GTXEN           | TXEN              |
| PD02    | GTX0            | TXD0              |
| PD03    | GTX1            | TXD1              |
| PD04    | GRXDV           | RXDV              |
| PD05    | GRX0            | RXD0              |
| PD06    | GRX1            | RXD1              |
| PD07    | GRXER           | RXER              |
| PD08    | GMDC            | MDC               |
| PD09    | GMDIO           | MDIO              |
| PC10    | GPIO            | nRST              |
| PA29    | GPIO            | SIGDET            |
| PD21    | SPI0_MOSI       | MOSI              |
| PD20    | SPI0_MISO       | MISO              |
| PD22    | SPI0_SPCK       | SCLK              |
| PC19    | ISI_PWD         | CS                |
| PA19    | GPIO            | GPIO0 (N/C?)      |
| PD28    | WKUP5           | GPIO1             |
| PA02    | WKUP2           | GPIO2             |

## Clock and Pin Notes

Clock tree is described on page 267 of the datasheet.

I can probably skip the SLCK "Slow clock 32.768 kHz" for now

MAINCK "Main Clock" is probably what I need, which is driven by an external 12MHz crystal, and is fed to the PLLACK "PLLA Clock". I'm not sure what feeds GPIO/Timers/Ethernet yet.

> After reset, the Main RC oscillator is enabled with the 12 MHz frequency selected. This oscillator is selected as the source of MAINCK. MAINCK is the default clock selected to start the system.

Page 273 talks about programming the PLLA Div/Mul, including "wait for lock" steps and such.

Page 275 talks about the Power Management Controller (PMC), which describes Master Clock (MCK), used by the flash controller, Processor Clock (HCLK) which is on whenever the CPU is not sleeping, Free Running processessor clock (FCLK) used for ???, and Generic Clock (GCLK) which is "provided to select peripherals"

> The free-running Processor clock (FCLK) used for sampling interrupts and clocking debug blocks ensures that interrupts can be sampled, and sleep events can be traced, while the processor is sleeping.

Page 276 shows the clock tree, and is likely (the beginning) of what I need to get the clocks configured correctly. Page 285 lists the "Recommended Programming Sequence".

> Note: USB, GMAC and MLB do not require PCKx to operate independently of core and bus peripherals.

> The PMC controls the clocks of the embedded peripherals by means of the Peripheral Control register (PMC_PCR). With this register, the user can enable and disable the different clocks used by the peripherals

Theres definitely some huge list of "Peripheral Identifiers", PIDx, which seems to be the way you enable/disable individual clocks to peripherals. I haven't found a mapping for this yet.

Ahh! These are on Page 66/Chapter 14.1, "Peripheral Identifiers"

There seems to be some "important" ones, that I hope are enabled by default in the range 0..=6, for core peripherals, and some one in the higher range for FPU exceptions? Unsure if these are necessary...

Useful identifiers:

10      PIOA
11      PIOB
12      PIOC
16      PIOD
17      PIOE
21      SPI0
23-25   Timer 0
26-28   Timer 1
39      GMAC
47-49   Timer 2
50-52   Timer 3
56      AES
57      TRNG
66      GMAC-Q1
67      GMAC-Q2
71      GMAC-Q3
72      GMAC-Q4
73      GMAC-Q5

## GPIO

There is a complicated diagram on page 342 explaining all the status registers that affect a GPIO pad.
