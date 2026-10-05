//! The Adjust panel, after Rerun's selection panel: collapsible sections of
//! property rows (label, control, value), with the rarely needed controls in
//! a closed "Advanced" tier. Each label explains itself in a tooltip. Lens,
//! field ROI and diagnostics join in Modules 4 and 5 (diagnostics in their
//! own drawer).

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    let Value = RecoText{width: theme.reco_value_width align: Align{x: 1.0}}
    // A checkbox starts where a slider's track does, after the knob's room.
    let Check = RecoCheckBox{margin: Inset{left: theme.reco_slider_knob}}

    mod.widgets.RecoInspector = SolidView{
        width: Fill height: Fill flow: Down
        draw_bg.color: theme.reco_panel
        RecoTitleRow{
            adjust_header := RecoStrong{text: "Adjust"}
        }
        ScrollShadowView{
            width: Fill height: Fill flow: Down
            fade_left: false
            fade_right: false
            view_section := RecoSection{
                header +: {
                    title +: {text: "View"}
                    help +: {text: "How the preview frames the pitch. Drag the picture to look around."}
                    Tip{text: "Reset view"
                        reset_view := RecoRowIcon{
                            draw_icon +: {svg: crate_resource("self:resources/icons/reset.svg")}
                        }
                    }
                }
                body +: {
                    RecoRow{
                        Tip{text: "Wider shows more of the pitch; narrower zooms in."
                            fov_label := RecoLabelCell{RecoSubdued{text: "Field of view"}}
                        }
                        fov_slider := RecoSlider{min: 20.0 max: 150.0 default: 75.0}
                        fov_value := Value{text: "75°"}
                    }
                    view_advanced := RecoAdvanced{
                        body +: {
                            RecoRow{
                                Tip{text: "Stops the view from turning past the edges of the stitched picture."
                                    RecoLabelCell{RecoSubdued{text: "Stay inside"}}
                                }
                                constrained_look := Check{text: "" animator +: {active: {default: @on}}}
                            }
                        }
                    }
                }
            }
            stitch_section := RecoSection{
                header +: {
                    title +: {text: "Stitch"}
                    help +: {text: "How the two cameras' pictures are joined into one."}
                }
                body +: {
                    RecoRow{
                        Tip{text: "Evens out brightness and colour between the two cameras."
                            match_label := RecoLabelCell{RecoSubdued{text: "Match colours"}}
                        }
                        match_colours := Check{text: "" animator +: {active: {default: @on}}}
                    }
                    RecoRow{
                        Tip{text: "How softly the two pictures merge where they meet."
                            seam_label := RecoLabelCell{RecoSubdued{text: "Seam blend"}}
                        }
                        seam_blend := RecoSlider{min: 0.0 max: 0.3 default: 0.05}
                        seam_value := Value{text: "0.05"}
                    }
                    stitch_advanced := RecoAdvanced{
                        body +: {
                            RecoRow{
                                Tip{text: "Levels the picture if the pole leans forward or back."
                                    RecoLabelCell{RecoSubdued{text: "Tilt"}}
                                }
                                rig_tilt := RecoSlider{min: -30.0 max: 30.0 default: 0.0}
                                tilt_value := Value{text: "0.0°"}
                            }
                            RecoRow{
                                Tip{text: "Levels the horizon if the pole leans to one side."
                                    RecoLabelCell{RecoSubdued{text: "Roll"}}
                                }
                                rig_roll := RecoSlider{min: -15.0 max: 15.0 default: 0.0}
                                roll_value := Value{text: "0.0°"}
                            }
                            RecoRow{
                                Tip{text: "Frames between the cameras: positive when the right camera started first."
                                    RecoLabelCell{RecoSubdued{text: "Sync (frames)"}}
                                }
                                sync_input := TextInput{
                                    width: Fill height: theme.reco_button margin: 0
                                    empty_text: "0"
                                }
                                sync_apply := RecoButton{text: "Apply"}
                            }
                            RecoRow{
                                Tip{text: "How far the two cameras' pictures overlap."
                                    RecoLabelCell{RecoSubdued{text: "Overlap"}}
                                }
                                intersect := RecoSlider{min: -1.0 max: 1.0 default: 0.0}
                                intersect_value := Value{text: "0.000"}
                            }
                            RecoRow{
                                Tip{text: "Where the virtual camera stands between the two."
                                    RecoLabelCell{RecoSubdued{text: "Camera depth"}}
                                }
                                axis_offset := RecoSlider{min: -0.6 max: 0.6 default: 0.0}
                                axis_value := Value{text: "0.000"}
                            }
                            RecoRow{
                                Tip{text: "Lifts the right camera's picture to meet the left."
                                    RecoLabelCell{RecoSubdued{text: "Vertical shift"}}
                                }
                                x_ty := RecoSlider{min: -0.1 max: 0.1 default: 0.0}
                                x_ty_value := Value{text: "0.000"}
                            }
                            RecoRow{
                                View{width: Fill height: Fit}
                                Tip{text: "Back to the overlap, depth and shift in the calibration file."
                                    reset_layout := RecoFlatButton{text: "Reset layout"}
                                }
                            }
                        }
                    }
                }
            }
            lens_section := RecoSection{
                header +: {
                    title +: {text: "Lens"}
                    help +: {text: "How each camera's fisheye picture is straightened. Fine-tune only if straight lines still bend."}
                    Tip{text: "Lens profiles"
                        lens_browse := RecoRowIcon{
                            draw_icon +: {svg: crate_resource("self:resources/icons/aperture.svg")}
                        }
                    }
                }
                body +: {
                    RecoRow{
                        RecoLabelCell{RecoSubdued{text: "Left"}}
                        left_lens_name := RecoMeta{text: ""}
                    }
                    RecoRow{
                        RecoLabelCell{RecoSubdued{text: "Right"}}
                        right_lens_name := RecoMeta{text: ""}
                    }
                    RecoRow{
                        Tip{text: "Straightens each camera's fisheye picture before stitching."
                            RecoLabelCell{RecoSubdued{text: "Correction"}}
                        }
                        lens_correction := Check{text: "" animator +: {active: {default: @on}}}
                    }
                    RecoRow{
                        Tip{text: "Shows one camera on its own, flat, to judge its lens."
                            RecoLabelCell{RecoSubdued{text: "Show"}}
                        }
                        lens_preview := RecoDropDown{width: Fill labels: ["Stitched picture" "Left camera" "Right camera"]}
                    }
                    lens_advanced := RecoAdvanced{
                        header +: {title +: {text: "Fine-tune"}}
                        body +: {
                            RecoRow{
                                Tip{text: "Which camera the sliders tune; Both moves the two in step."
                                    RecoLabelCell{RecoSubdued{text: "Camera"}}
                                }
                                lens_camera := RecoDropDown{width: Fill labels: ["Left" "Right" "Both"]}
                            }
                            RecoRow{
                                Tip{text: "The lens's focal length across the picture, in pixels."
                                    RecoLabelCell{RecoSubdued{text: "Focal x"}}
                                }
                                lens_fx := RecoSlider{min: 0.0 max: 1.0 default: 0.5}
                                lens_fx_value := Value{text: ""}
                            }
                            RecoRow{
                                Tip{text: "The lens's focal length up the picture, in pixels."
                                    RecoLabelCell{RecoSubdued{text: "Focal y"}}
                                }
                                lens_fy := RecoSlider{min: 0.0 max: 1.0 default: 0.5}
                                lens_fy_value := Value{text: ""}
                            }
                            RecoRow{
                                Tip{text: "Where the lens's axis meets the picture, across, in pixels."
                                    RecoLabelCell{RecoSubdued{text: "Centre x"}}
                                }
                                lens_cx := RecoSlider{min: 0.0 max: 1.0 default: 0.5}
                                lens_cx_value := Value{text: ""}
                            }
                            RecoRow{
                                Tip{text: "Where the lens's axis meets the picture, up, in pixels."
                                    RecoLabelCell{RecoSubdued{text: "Centre y"}}
                                }
                                lens_cy := RecoSlider{min: 0.0 max: 1.0 default: 0.5}
                                lens_cy_value := Value{text: ""}
                            }
                            RecoRow{
                                Tip{text: "The first fisheye distortion term."
                                    RecoLabelCell{RecoSubdued{text: "k1"}}
                                }
                                lens_k1 := RecoSlider{min: 0.0 max: 1.0 default: 0.5}
                                lens_k1_value := Value{text: ""}
                            }
                            RecoRow{
                                Tip{text: "The second fisheye distortion term."
                                    RecoLabelCell{RecoSubdued{text: "k2"}}
                                }
                                lens_k2 := RecoSlider{min: 0.0 max: 1.0 default: 0.5}
                                lens_k2_value := Value{text: ""}
                            }
                            RecoRow{
                                Tip{text: "The third fisheye distortion term."
                                    RecoLabelCell{RecoSubdued{text: "k3"}}
                                }
                                lens_k3 := RecoSlider{min: 0.0 max: 1.0 default: 0.5}
                                lens_k3_value := Value{text: ""}
                            }
                            RecoRow{
                                Tip{text: "The fourth fisheye distortion term."
                                    RecoLabelCell{RecoSubdued{text: "k4"}}
                                }
                                lens_k4 := RecoSlider{min: 0.0 max: 1.0 default: 0.5}
                                lens_k4_value := Value{text: ""}
                            }
                            RecoRow{
                                View{width: Fill height: Fit}
                                Tip{text: "Back to the lenses in the calibration file."
                                    reset_lens := RecoFlatButton{text: "Reset lens"}
                                }
                            }
                        }
                    }
                }
            }
            stats_section := RecoSection{
                animator +: {active: {default: @off}}
                header +: {
                    fold_button +: {animator +: {active: {default: @off}}}
                    title +: {text: "Stats"}
                    help +: {text: "How fast the preview runs, and how well the cameras were calibrated."}
                }
                body +: {
                    RecoRow{
                        Tip{text: "Frames the preview shows a second."
                            RecoLabelCell{RecoSubdued{text: "Frame rate"}}
                        }
                        stats_fps := RecoMeta{text: "—"}
                    }
                    RecoRow{
                        Tip{text: "How long a frame takes on average."
                            RecoLabelCell{RecoSubdued{text: "Frame time"}}
                        }
                        stats_frame := RecoMeta{text: "—"}
                    }
                    RecoRow{
                        Tip{text: "The slowest frame in a hundred."
                            RecoLabelCell{RecoSubdued{text: "Slowest 1%"}}
                        }
                        stats_slowest := RecoMeta{text: "—"}
                    }
                    RecoRow{
                        Tip{text: "How long the preview waited for the next frame pair from the decoders (they read ahead)."
                            RecoLabelCell{RecoSubdued{text: "Decode wait"}}
                        }
                        stats_decode := RecoMeta{text: "—"}
                    }
                    RecoRow{
                        Tip{text: "Stitching the frame and handing it to the window."
                            RecoLabelCell{RecoSubdued{text: "Render"}}
                        }
                        stats_render := RecoMeta{text: "—"}
                    }
                    RecoRow{
                        Tip{text: "The graphics processor the preview runs on."
                            RecoLabelCell{RecoSubdued{text: "GPU"}}
                        }
                        stats_gpu := RecoMeta{text: "—"}
                    }
                    RecoRow{
                        Tip{text: "How well the last calibration matched the two cameras."
                            RecoLabelCell{RecoSubdued{text: "Calibration"}}
                        }
                        stats_calibration := RecoMeta{text: "—"}
                    }
                }
            }
        }
    }
}
