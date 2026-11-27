N10 (Summary, written by PutiaCNC)
N20 (Spindle (!):            Not run by the program: it stops -M0- to ask you to start it at the)
N30 (                        speed shown, and again before a tool change and at the end to stop it.)
N40 (                        There is no work coordinate system: jog to the zero of the job and set)
N50 (                        it with G92 X0 Y0 Z0 first.)
N60 (Tools:                  6 mm flat end mill. No tool change.)
N70 (Pauses:                 The program stops 2 times with the spindle off: 'Start the spindle at)
N80 (                        10000 rpm, then resume', 'Stop the spindle, then resume'. Resume it)
N90 (                        when you are ready; the spindle starts again by itself.)
N100 (Cutting:                Down to Z -1 mm. 33 mm of cutting, about 5 s in all.)
N110 (End:                    Lifts to Z 5 mm, goes to X0 Y0.)
N120 (File 2 of 2: 6 mm flat end mill operation 2)
N130 (Before you start: change to 6 mm flat end mill and touch off Z0 again.)
N140 ; PutiaCNC - Marlin (no spindle control)
N150 ; Stock 60 x 40 x 6 mm; XY0 = lower-left corner of stock, Z0 = top surface
N160 G21 G90
N170 ; Marlin: jog the tool to the zero of the job and set it there with G92 X0 Y0 Z0 before you run this file
N180 ; Spindle: by hand. The program stops to ask you to start and stop it
N190 ; start code: dust shoe down
N200 M8
N210 G4 P1
N220 G0 Z5
N230 ; Tool: 6 mm flat end mill
N240 M0 Start the spindle at 10000 rpm, then resume
N250 ; Operation 2: inside, tool 6 mm
N260 G0 X44.618 Y20.015
N270 G0 Z0.5
N280 G1 Z0 F300
N290 ; Pass 1
N300 G1 Z-1
N310 G1 F1000
N320 G1 X44.122 Y20.078
N330 G1 X43.645 Y20.187
N340 G1 X43.153 Y20.354
N350 G1 X42.714 Y20.553
N360 G1 X42.282 Y20.803
N370 G1 X41.877 Y21.095
N380 G1 X41.492 Y21.437
N390 G1 X41.146 Y21.815
N400 G1 X40.875 Y22.174
N410 G1 X40.604 Y22.617
N420 G1 X40.377 Y23.095
N430 G1 X40.222 Y23.528
N440 G1 X40.101 Y24.005
N450 G1 X40.001 Y24.884
N460 G1 X40.015 Y25.382
N470 G1 X40.078 Y25.878
N480 G1 X40.187 Y26.355
N490 G1 X40.354 Y26.847
N500 G1 X40.553 Y27.286
N510 G1 X40.803 Y27.718
N520 G1 X41.095 Y28.123
N530 G1 X41.437 Y28.508
N540 G1 X41.782 Y28.826
N550 G1 X42.174 Y29.125
N560 G1 X42.617 Y29.396
N570 G1 X43.079 Y29.616
N580 G1 X43.528 Y29.778
N590 G1 X44.022 Y29.903
N600 G1 X44.884 Y29.999
N610 G1 X45.382 Y29.985
N620 G1 X45.878 Y29.922
N630 G1 X46.355 Y29.813
N640 G1 X46.847 Y29.646
N650 G1 X47.286 Y29.447
N660 G1 X47.718 Y29.197
N670 G1 X48.123 Y28.905
N680 G1 X48.508 Y28.563
N690 G1 X48.854 Y28.185
N700 G1 X49.125 Y27.826
N710 G1 X49.396 Y27.383
N720 G1 X49.623 Y26.905
N730 G1 X49.778 Y26.472
N740 G1 X49.903 Y25.978
N750 G1 X49.999 Y25.116
N760 G1 X49.984 Y24.601
N770 G1 X49.919 Y24.107
N780 G1 X49.813 Y23.645
N790 G1 X49.642 Y23.142
N800 G1 X49.447 Y22.714
N810 G1 X49.185 Y22.264
N820 G1 X48.905 Y21.877
N830 G1 X48.563 Y21.492
N840 G1 X48.196 Y21.156
N850 G1 X47.826 Y20.875
N860 G1 X47.383 Y20.604
N870 G1 X46.921 Y20.384
N880 G1 X46.472 Y20.222
N890 G1 X45.978 Y20.097
N900 G1 X45.116 Y20.001
N910 G1 X44.618 Y20.015
N920 G0 Z5
N930 M0 Stop the spindle, then resume
N940 G0 X0 Y0
N950 M9
N960 ; end code: dust shoe up
N970 ; End of program